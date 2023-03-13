//! mod statistics counts all relevant information about the server response

use num::integer::Roots;
use reqwest::{Error, Response, StatusCode};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::*};
use std::time::{Duration, Instant};
use tokio::{sync as tsync, time as ttime};

#[derive(Debug)]
pub(crate) struct Statistics {
    /// status code [100, 200)
    rsp1xx: AtomicU64,

    /// status code [200, 300)
    rsp2xx: AtomicU64,

    /// status code [300, 400)
    rsp3xx: AtomicU64,

    /// status code [400, 500)
    rsp4xx: AtomicU64,

    /// status code [500, 511]
    rsp5xx: AtomicU64,

    /// other response code
    rsp_others: AtomicU64,

    /// errors category
    errors: tsync::Mutex<HashMap<String, u64>>,

    /// start time
    started_at: tsync::Mutex<Instant>,

    /// total_success send and receive response requests
    total_success: AtomicU64,

    /// total send and receive response requests although meets error
    total: AtomicU64,

    /// maximum per second
    max_req_per_second: tsync::Mutex<f64>,

    /// average per second
    avg_req_per_second: tsync::Mutex<f64>,

    /// stdev per second, link: https://en.wikipedia.org/wiki/Standard_deviation
    stdev_per_second: tsync::Mutex<f64>,

    /// log requests by second
    req_per_second: tsync::Mutex<Vec<u64>>,

    /// used for internal statistics, the number of requests accumulated in the
    /// current second will be reset when the next second starts
    current_cumulative: AtomicU64,

    /// average time spent on request
    avg_req_elapsed_time: tsync::Mutex<Duration>,

    /// maximum time spent by the request
    max_req_elapsed_time: tsync::Mutex<Duration>,

    /// stdev per request, link: https://en.wikipedia.org/wiki/Standard_deviation
    stdev_req_elapsed_time: tsync::Mutex<Duration>,

    /// used internally to record the time spent on each request
    elapsed_time: tsync::Mutex<Vec<Duration>>,

    /// indicates whether to stop, used to notify the internal timer to exit
    is_stopped: AtomicBool,

    /// recording stop time
    stopped_at: tsync::Mutex<Option<Instant>>,

    /// throughput, connections / avg_req_elapsed_time, reqs/s
    throughput: tsync::Mutex<f64>,

    /// latencies for different percentiles
    latencies: tsync::Mutex<Vec<(f32, Duration)>>,
}

impl Statistics {
    /// construct empty Statistics
    pub(crate) fn new() -> Statistics {
        Self {
            rsp1xx: AtomicU64::new(0),
            rsp2xx: AtomicU64::new(0),
            rsp3xx: AtomicU64::new(0),
            rsp4xx: AtomicU64::new(0),
            rsp5xx: AtomicU64::new(0),
            rsp_others: AtomicU64::new(0),
            errors: tsync::Mutex::new(HashMap::new()),
            started_at: tsync::Mutex::new(Instant::now()),
            total: AtomicU64::new(0),
            total_success: AtomicU64::new(0),
            req_per_second: tsync::Mutex::new(Vec::new()),
            avg_req_per_second: tsync::Mutex::new(0.0),
            max_req_per_second: tsync::Mutex::new(0.0),
            stdev_per_second: tsync::Mutex::new(0.0),
            is_stopped: AtomicBool::new(false),
            current_cumulative: AtomicU64::new(0),
            stopped_at: tsync::Mutex::new(None),
            latencies: tsync::Mutex::new(Vec::new()),
            throughput: tsync::Mutex::new(0.0),
            elapsed_time: tsync::Mutex::new(Vec::new()),
            avg_req_elapsed_time: tsync::Mutex::new(Duration::from_secs(0)),
            max_req_elapsed_time: tsync::Mutex::new(Duration::from_secs(0)),
            stdev_req_elapsed_time: tsync::Mutex::new(Duration::from_secs(0)),
        }
    }

    /// return current send and rcv requests
    pub(crate) fn get_total(&self) -> u64 {
        self.total.load(Acquire)
    }

    /// if there will be a lot of preparation work before starting the
    /// statistics, it is best to reset the start time at the official start
    pub(crate) async fn reset_start_time(&self) {
        let mut started_at = self.started_at.lock().await;
        *started_at = Instant::now();
    }

    /// used to start the internal timer, and generate a box of snapshots for
    /// some data every second
    pub(crate) async fn timer_per_second(&self) {
        let mut timer = ttime::interval(Duration::from_secs(2));
        loop {
            timer.tick().await;
            {
                let mut req_per_second = self.req_per_second.lock().await;
                req_per_second.push(self.current_cumulative.load(Acquire));
                self.current_cumulative.store(0, SeqCst);
            }
            if self.is_stopped.load(Acquire) {
                break;
            }
        }
    }

    fn statistics_rsp_code(&self, status: StatusCode) {
        match status {
            status
                if status >= StatusCode::CONTINUE
                    && status < StatusCode::OK =>
            {
                self.rsp1xx.fetch_add(1, SeqCst);
            },
            status
                if status >= StatusCode::OK
                    && status < StatusCode::MULTIPLE_CHOICES =>
            {
                self.rsp2xx.fetch_add(1, SeqCst);
            },
            status
                if status >= StatusCode::MULTIPLE_CHOICES
                    && status < StatusCode::BAD_REQUEST =>
            {
                self.rsp3xx.fetch_add(1, SeqCst);
            },
            status
                if status >= StatusCode::BAD_REQUEST
                    && status < StatusCode::INTERNAL_SERVER_ERROR =>
            {
                self.rsp4xx.fetch_add(1, SeqCst);
            },
            status
                if status >= StatusCode::INTERNAL_SERVER_ERROR
                    && status
                        <= StatusCode::NETWORK_AUTHENTICATION_REQUIRED =>
            {
                self.rsp5xx.fetch_add(1, SeqCst);
            },
            _ => {
                self.rsp_others.fetch_add(1, SeqCst);
            },
        }
    }

    async fn handle_resp_error(&self, err: Error) {
        let err_msg = format!("{err}");
        let mut errors = self.errors.lock().await;
        errors
            .entry(err_msg)
            .and_modify(|count| *count += 1)
            .or_insert(1);
        if let Some(status) = err.status() {
            self.statistics_rsp_code(status);
        }
    }

    /// receive message and make statistics
    pub(crate) async fn handle_message(&self, message: Message) {
        let Message {
            rsp_at,
            req_at,
            response,
        } = message;

        self.total.fetch_add(1, SeqCst);

        if response.is_err() {
            let err = response.err().unwrap();
            self.handle_resp_error(err).await;
            return;
        }

        let response = response.unwrap();
        self.statistics_rsp_code(response.status());
        self.total_success.fetch_add(1, SeqCst);
        self.current_cumulative.fetch_add(1, SeqCst);
        let mut elapsed_time = self.elapsed_time.lock().await;
        elapsed_time.push(rsp_at - req_at);
    }

    /// notify stop timer
    pub(crate) async fn stop_timer(&self) {
        self.is_stopped.store(true, SeqCst);
        let mut stopped_at = self.stopped_at.lock().await;
        *stopped_at = Some(Instant::now());
    }

    async fn calculate_max_per_second(&self) {
        let req_per_second = self.req_per_second.lock().await;
        if let Some(max) = req_per_second.iter().max() {
            let mut max_per_second = self.max_req_per_second.lock().await;
            *max_per_second = *max as f64;
        }
    }

    async fn calculate_avg_per_second(&self) {
        let stopped_at = self.stopped_at.lock().await;
        let started_at = self.started_at.lock().await;
        if let Some(stopped_at) = *stopped_at {
            let delta = (stopped_at - *started_at).as_secs_f64();
            if delta == 0.0 {
                return;
            }
            let mut avg_per_second = self.avg_req_per_second.lock().await;
            *avg_per_second = self.total_success.load(SeqCst) as f64 / delta;
        }
    }

    async fn calculate_elapsed_time(&self) {
        let mut elapsed_time = self.elapsed_time.lock().await;
        if (*elapsed_time).is_empty() {
            return;
        }
        elapsed_time.sort();

        // avg_req_elapsed_time
        let mut avg_req_elapsed_time = self.avg_req_elapsed_time.lock().await;
        let total: Duration = elapsed_time.iter().sum();
        let count = elapsed_time.len();
        *avg_req_elapsed_time = total / count as u32;

        // max_req_elapsed_time
        let mut max_req_elapsed_time = self.max_req_elapsed_time.lock().await;
        if let Some(max) = elapsed_time.iter().max() {
            *max_req_elapsed_time = *max;
        }

        // stdev_req_elapsed_time
        let sum = (*elapsed_time).iter().sum::<Duration>();
        let mean = (sum as Duration / count as u32).as_nanos();
        let variance: u128 = (*elapsed_time)
            .iter()
            .map(|x| {
                let diff: i128 = (*x).as_nanos() as i128 - mean as i128;
                (diff * diff) as u128
            })
            .sum::<u128>()
            / count as u128;
        let stdev = variance.sqrt();
        let mut stdev_req_elapsed_time =
            self.stdev_req_elapsed_time.lock().await;
        *stdev_req_elapsed_time = Duration::from_nanos(stdev as u64);
    }

    async fn calculate_stdev_per_second(&self) {
        let req_per_second = self.req_per_second.lock().await;
        if (*req_per_second).is_empty() {
            return;
        }

        let mut origin = &*req_per_second as &[u64];

        // break off both ends
        if origin[0] == 0 {
            origin = &origin[1..];
        }
        if origin.len() >= 2 {
            origin = &origin[..origin.len() - 1];
        }

        let count = origin.len();
        let sum = origin.iter().sum::<u64>();
        let mean = sum as f64 / count as f64;
        let variance = origin
            .iter()
            .map(|x| {
                let diff = *x as f64 - mean;
                diff * diff
            })
            .sum::<f64>()
            / count as f64;
        let mut stdev_per_second = self.stdev_per_second.lock().await;
        *stdev_per_second = variance.sqrt();
    }

    async fn calculate_throughput(&self, connections: u16) {
        let avg_req_elapsed_time = self.avg_req_elapsed_time.lock().await;
        let mut throughput = self.throughput.lock().await;
        let sec = (*avg_req_elapsed_time).as_secs_f64();
        *throughput = connections as f64 / sec;
    }

    async fn calculate_latencies(&self, percentiles: Vec<f32>) {
        let mut elapsed_time = self.elapsed_time.lock().await;
        if elapsed_time.is_empty() {
            return;
        }
        if !elapsed_time.is_sorted() {
            elapsed_time.sort();
        }

        let mut latencies = self.latencies.lock().await;
        let count = elapsed_time.len();
        for percent in percentiles {
            let percent_len = (count as f32 * percent) as usize;
            if percent_len > count {
                continue;
            }
            let percent_elapsed_time: &[Duration] =
                &(*elapsed_time)[..percent_len];
            let sum = percent_elapsed_time.iter().sum::<Duration>();
            latencies.push((percent, sum / percent_len as u32));
        }
    }

    async fn clear_temporary_data(&self) {
        let mut elapsed_time = self.elapsed_time.lock().await;
        elapsed_time.clear();
        elapsed_time.shrink_to(0);
    }

    /// need to manually call this method for statistical summary
    pub(crate) async fn summary(
        &self,
        connections: u16,
        percentiles: Vec<f32>,
    ) {
        self.calculate_max_per_second().await;
        self.calculate_avg_per_second().await;
        self.calculate_elapsed_time().await;
        self.calculate_stdev_per_second().await;
        self.calculate_throughput(connections).await;
        self.calculate_latencies(percentiles).await;
        self.clear_temporary_data().await;
    }
}

impl Default for Statistics {
    fn default() -> Self {
        Statistics::new()
    }
}

/// Message entity for [Statistics]
pub struct Message {
    rsp_at: Instant,
    req_at: Instant,
    response: Result<Response, Error>,
}

impl Message {
    /// construct message
    pub fn new(
        response: Result<Response, Error>,
        req_at: Instant,
        rsp_at: Instant,
    ) -> Message {
        Self {
            rsp_at,
            req_at,
            response,
        }
    }
}
