//! mod statistics counts all relevant information about the server response

use reqwest::{Error, Response, StatusCode};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::*};
use std::time::{Duration, Instant};
use tokio::{sync as tsync, time as ttime};

pub struct Statistics {
    rsp1xx: AtomicU64,
    rsp2xx: AtomicU64,
    rsp3xx: AtomicU64,
    rsp4xx: AtomicU64,
    rsp5xx: AtomicU64,
    rsp_others: AtomicU64,
    errors: tsync::Mutex<HashMap<String, u64>>,

    start: Instant,
    total: AtomicU64,
    max_per_second: tsync::Mutex<f64>,
    avg_per_second: tsync::Mutex<f64>,
    req_per_second: tsync::Mutex<Vec<u64>>,
    current_cumulative: AtomicU64,

    min_elapsed_time: tsync::Mutex<Duration>,
    max_elapsed_time: tsync::Mutex<Duration>,
    elapsed_time: tsync::Mutex<Vec<Duration>>,

    is_stopped: AtomicBool,
    stopped_at: tsync::Mutex<Option<Instant>>,
}

impl Statistics {
    pub fn new() -> Statistics {
        Self {
            rsp1xx: AtomicU64::new(0),
            rsp2xx: AtomicU64::new(0),
            rsp3xx: AtomicU64::new(0),
            rsp4xx: AtomicU64::new(0),
            rsp5xx: AtomicU64::new(0),
            rsp_others: AtomicU64::new(0),
            errors: tsync::Mutex::new(HashMap::new()),
            start: Instant::now(),
            total: AtomicU64::new(0),
            req_per_second: tsync::Mutex::new(Vec::new()),
            avg_per_second: tsync::Mutex::new(0.0),
            max_per_second: tsync::Mutex::new(0.0),
            is_stopped: AtomicBool::new(false),
            current_cumulative: AtomicU64::new(0),
            stopped_at: tsync::Mutex::new(None),
            elapsed_time: tsync::Mutex::new(Vec::new()),
            min_elapsed_time: tsync::Mutex::new(Duration::from_secs(0)),
            max_elapsed_time: tsync::Mutex::new(Duration::from_secs(0)),
        }
    }

    pub async fn tick_per_second(&self) {
        let mut timer = ttime::interval(Duration::from_secs(2));
        loop {
            timer.tick().await;
            {
                let mut req_per_second = self.req_per_second.lock().await;
                req_per_second.push(self.current_cumulative.load(SeqCst));
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
        let err_msg = format!("{}", err);
        let mut errors = self.errors.lock().await;
        errors
            .entry(err_msg)
            .and_modify(|count| *count += 1)
            .or_insert(1);
        if let Some(status) = err.status() {
            self.statistics_rsp_code(status);
        }
    }

    pub async fn handle_message(&self, message: Message) {
        let Message {
            rsp_at,
            req_at,
            response,
        } = message;

        if response.is_err() {
            let err = response.err().unwrap();
            self.handle_resp_error(err).await;
            return;
        }

        let response = response.unwrap();
        self.statistics_rsp_code(response.status());
        self.total.fetch_add(1, SeqCst);
        self.current_cumulative.fetch_add(1, SeqCst);
        let mut elapsed_time = self.elapsed_time.lock().await;
        elapsed_time.push(rsp_at - req_at);
    }

    pub async fn stop_tick(&self) {
        self.is_stopped.store(true, SeqCst);
        let mut stopped_at = self.stopped_at.lock().await;
        *stopped_at = Some(Instant::now());
    }

    async fn calculate_max_per_second(&self) {
        let req_per_second = self.req_per_second.lock().await;
        if let Some(max) = req_per_second.iter().max() {
            let mut max_per_second = self.max_per_second.lock().await;
            *max_per_second = *max as f64;
        }
    }

    async fn calculate_avg_per_second(&self) {
        let mut stopped_at = self.stopped_at.lock().await;
        if let Some(stopped_at) = *stopped_at {
            let delta = (stopped_at - self.start).as_secs_f64();
            if delta == 0.0 {
                return;
            }
            let mut avg_per_second = self.avg_per_second.lock().await;
            *avg_per_second = self.total.load(SeqCst) as f64 / delta;
        }
    }

    async fn calculate_elapsed_time(&self) {
        let mut elapsed_time = self.elapsed_time.lock().await;
        elapsed_time.sort();

        let mut min_elapsed_time = self.min_elapsed_time.lock().await;
        if let Some(min) = elapsed_time.iter().min() {
            *min_elapsed_time = *min;
        }

        let mut max_elapsed_time = self.max_elapsed_time.lock().await;
        if let Some(max) = elapsed_time.iter().min() {
            *max_elapsed_time = *max;
        }

        // clear after using
        elapsed_time.clear();
        elapsed_time.shrink_to(0);
    }

    pub async fn summary(&self) {
        self.calculate_max_per_second().await;
        self.calculate_avg_per_second().await;
        self.calculate_elapsed_time().await;
    }
}

impl Default for Statistics {
    fn default() -> Self {
        Statistics::new()
    }
}

pub struct Message {
    rsp_at: Instant,
    req_at: Instant,
    response: Result<Response, Error>,
}

impl Message {
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
