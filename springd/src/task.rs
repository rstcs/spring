use crate::client::build_client;
use crate::dispatcher::DurationDispatcher;
use crate::dispatcher::{CountDispatcher, Dispatcher};
use crate::request::build_request;
use crate::statistics::{Message, Statistics};
use crate::Arg;
use indicatif::ProgressBar;
use log::error;
use num_cpus;
use reqwest::Client;
use std::cmp::min;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::{
    self, runtime,
    sync::{self as tsync, mpsc},
};

pub struct Task {
    arg: Arg,
    client: Client,
    statistics: Statistics,
    workers_done: AtomicBool,
    progress_bar: Option<ProgressBar>,
    dispatcher: Arc<tsync::RwLock<Box<dyn Dispatcher>>>,
}

fn create_count_dispatcher(
    total: u64,
    rate: &Option<u16>,
) -> Box<dyn Dispatcher> {
    let count_dispatcher = CountDispatcher::new(total, rate);
    Box::new(count_dispatcher)
}

fn create_duration_dispatcher(
    duration: Duration,
    rate: &Option<u16>,
) -> Box<dyn Dispatcher> {
    let duration_dispatcher = DurationDispatcher::new(duration, rate);
    Box::new(duration_dispatcher)
}

fn create_dispatcher(arg: &Arg) -> Arc<tsync::RwLock<Box<dyn Dispatcher>>> {
    let dispatcher = if arg.requests.is_some() {
        Arc::new(tsync::RwLock::new(create_count_dispatcher(
            arg.requests.unwrap(),
            &arg.rate,
        )))
    } else {
        Arc::new(tsync::RwLock::new(create_duration_dispatcher(
            arg.duration.unwrap(),
            &arg.rate,
        )))
    };
    dispatcher
}

impl Task {
    /// construct new task
    pub fn new(
        arg: Arg,
        progress_bar: Option<ProgressBar>,
    ) -> anyhow::Result<Self> {
        let client = build_client(&arg)?;
        let dispatcher = create_dispatcher(&arg);

        Ok(Self {
            arg,
            client,
            dispatcher,
            progress_bar,
            statistics: Statistics::new(),
            workers_done: AtomicBool::new(false),
        })
    }

    async fn update_progress_bar(self: Arc<Self>) {
        if self.progress_bar.is_none() {
            return;
        }
        if self.arg.requests.is_some() {
            self.update_count_progress_bar().await;
        } else if self.arg.duration.is_some() {
            self.update_duration_progress_bar().await;
        }
    }

    async fn update_count_progress_bar(self: Arc<Self>) {
        let total = self.arg.requests.unwrap();
        loop {
            self.progress_bar
                .clone()
                .unwrap()
                .set_position(min(self.statistics.get_total(), total));
            if self.workers_done.load(Ordering::Acquire) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    async fn update_duration_progress_bar(self: Arc<Self>) {
        let total = self.arg.duration.unwrap().as_secs();
        let mut current = 0;
        loop {
            current += 1;
            self.progress_bar
                .clone()
                .unwrap()
                .set_position(min(current, total));
            if self.workers_done.load(Ordering::Acquire) {
                break;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    fn finish_progress_bar(self: Arc<Self>) {
        if let Some(progress_bar) = &self.progress_bar {
            if !progress_bar.is_finished() {
                progress_bar.finish();
            }
        }
    }

    async fn worker(self: Arc<Self>, sender: mpsc::Sender<Message>) {
        loop {
            if !self.dispatcher.read().await.try_apply_job().await {
                break;
            }

            let request = build_request(&self.arg, &self.client).await;
            if request.is_err() {
                panic!("build request error: {:?}", request.err());
            }

            let req_at = Instant::now();
            let response = self.client.execute(request.unwrap()).await;
            self.dispatcher.read().await.complete_job();
            let message = Message::new(response, req_at, Instant::now());
            let _ = sender.send(message).await;
        }
    }

    async fn rcv_worker_message(
        self: Arc<Self>,
        mut receiver: mpsc::Receiver<Message>,
    ) {
        loop {
            let result = receiver.try_recv();
            if result.is_ok() {
                self.statistics.handle_message(result.unwrap()).await;
                continue;
            }
            if self.workers_done.load(Ordering::Acquire) {
                break;
            }
            tokio::time::sleep(Duration::from_nanos(100)).await;
        }
    }

    /// run task and make statistics
    pub fn run(self: Arc<Self>) -> anyhow::Result<()> {
        let rt = runtime::Builder::new_multi_thread()
            .worker_threads(num_cpus::get())
            .thread_name("springd-tokio-runtime-worker")
            .unhandled_panic(runtime::UnhandledPanic::ShutdownRuntime)
            .enable_all()
            .build()?;

        rt.block_on(async {
            let (tx, mut rx) = mpsc::channel::<Message>(500);

            // start workers by connection number
            let mut jobs = Vec::with_capacity(self.arg.connections as usize);

            // reset start time
            let task = self.clone();
            tokio::spawn(async move {
                task.statistics.reset_start_time().await;
            })
            .await
            .expect("reset statistics start time failed");

            // handle statistics
            let statistics_job =
                tokio::spawn(self.clone().rcv_worker_message(rx));

            // update progress bar job
            let update_pb_job =
                tokio::spawn(self.clone().update_progress_bar());

            // start all worker and send request
            for _ in 0..self.arg.connections {
                jobs.push(tokio::spawn(self.clone().worker(tx.clone())));
            }

            // start statistics timer
            let task = self.clone();
            let stat_timer = tokio::spawn(async move {
                task.statistics.timer_per_second().await;
            });

            // wait all jobs end
            for worker in jobs {
                let result = worker.await;
                if result.is_err() {
                    error!(
                        "worker execute request failed: {:?}",
                        result.unwrap_err()
                    );
                }
            }
            self.workers_done.store(true, Ordering::SeqCst);

            // notify stop statics timer
            let task = self.clone();
            tokio::spawn(async move {
                task.statistics.stop_timer().await;
            })
            .await
            .expect("notify stop statistics timer failed");

            // wait statistics job complete
            statistics_job.await.expect("statistics job failed");

            // wait update progress bar job finish
            update_pb_job.await.expect("update progress bar job failed");

            // wait statistics timer end
            stat_timer.await.expect("statistics timer tun failed");

            // finish progress bar
            self.clone().finish_progress_bar();

            // wait statistics summary
            let task = self.clone();
            tokio::spawn(async move {
                task.statistics
                    .summary(task.arg.connections, task.arg.percentiles.clone())
                    .await;
            })
            .await
            .expect("statistics summary failed");

            error!("{:#?}", self.statistics);
        });

        Ok(())
    }
}
