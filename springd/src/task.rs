use crate::client::build_client;
use crate::dispatcher::DurationDispatcher;
use crate::dispatcher::{CountDispatcher, Dispatcher};
use crate::request::build_request;
use crate::statistics::{Message, Statistics};
use crate::Arg;
use log::error;
use num_cpus;
use reqwest::Client;
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
    dispatcher_lock: Arc<tsync::RwLock<Box<dyn Dispatcher>>>,
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

impl Task {
    pub fn new(arg: Arg) -> anyhow::Result<Self> {
        let client = build_client(&arg)?;
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

        Ok(Self {
            arg,
            client,
            statistics: Statistics::new(),
            workers_done: AtomicBool::new(false),
            dispatcher_lock: dispatcher,
        })
    }

    async fn worker(self: Arc<Self>, sender: mpsc::Sender<Message>) {
        loop {
            if !self.dispatcher_lock.read().await.try_apply_job().await {
                break;
            }

            let request = build_request(&self.arg, &self.client).await;
            if request.is_err() {
                panic!("build request error: {:?}", request.err());
            }

            let req_at = Instant::now();
            let response = self.client.execute(request.unwrap()).await;
            self.dispatcher_lock.read().await.complete_job();
            let message = Message::new(response, req_at, Instant::now());
            let _ = sender.send(message).await;
        }
    }

    async fn statistics(
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
            tokio::time::sleep(Duration::from_nanos(10)).await;
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
            let task_copy = self.clone();
            tokio::spawn(async move {
                task_copy.statistics.reset_start_time().await;
            })
            .await
            .expect("reset statistics start time failed");

            // handle statistics
            let statistics_job = tokio::spawn(self.clone().statistics(rx));

            // start all worker and send request
            for _ in 0..self.arg.connections {
                jobs.push(tokio::spawn(self.clone().worker(tx.clone())));
            }

            // start statistics timer
            let task_copy = self.clone();
            let stat_timer = tokio::spawn(async move {
                task_copy.statistics.timer_per_second().await;
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
            let task_copy = self.clone();
            tokio::spawn(async move {
                task_copy.statistics.stop_timer().await;
            })
            .await
            .expect("notify stop statistics timer failed");

            // wait statistics job complete
            statistics_job.await.expect("statistics job failed");

            // wait statistics timer end
            stat_timer.await.expect("statistics timer tun failed");

            // wait statistics summary
            let task_copy = self.clone();
            tokio::spawn(async move {
                task_copy
                    .statistics
                    .summary(
                        task_copy.arg.connections,
                        task_copy.arg.percentiles.clone(),
                    )
                    .await;
            })
            .await
            .expect("statistics summary failed");

            error!("{:#?}", self.statistics);
        });

        Ok(())
    }
}
