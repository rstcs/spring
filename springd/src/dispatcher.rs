//! dispatcher module is used to distribute tasks according to different models

use crate::limiter::Limiter;
use async_trait::async_trait;
use log::{debug, error, info};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::*};
use std::time::{Duration, Instant};

#[async_trait]
pub trait Dispatcher: Send + Sync {
    /// query current task process, returning 0 to 1
    fn get_process(&self) -> f64;

    /// worker apply a job from dispatcher, return true continue to handle,
    /// return false worker will exit.
    async fn try_apply_job(&self) -> bool;

    /// when worker complete job, it will notify the dispatcher
    fn complete_job(&self);

    /// when the program receives an external termination signal, notify the
    /// Dispatcher to process it
    fn cancel(&mut self);
}

/// [CountDispatcher] is a count based task dispatcher
pub struct CountDispatcher {
    /// total requests number will send to server
    total: u64,

    /// number of jobs already applied for
    applied: AtomicU64,

    /// the amount of work done
    completed: AtomicU64,

    /// indicates whether it is canceled
    is_canceled: AtomicBool,

    /// indicate whether to complete
    is_done: AtomicBool,

    /// a rate limiter that limits the acquisition of a fixed number of tokens
    /// per second
    limiter: Option<Limiter>,
}

fn new_limiter(rate: &Option<u16>) -> Option<Limiter> {
    let mut limiter: Option<Limiter> = None;
    if let Some(rate) = rate {
        limiter = Some(Limiter::new(*rate));
    }
    limiter
}

impl CountDispatcher {
    /// give total and rat, return [Dispatcher]
    pub fn new(total: u64, rate: &Option<u16>) -> Self {
        Self {
            total,
            limiter: new_limiter(rate),
            applied: AtomicU64::new(0),
            completed: AtomicU64::new(0),
            is_canceled: AtomicBool::new(false),
            is_done: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl Dispatcher for CountDispatcher {
    fn get_process(&self) -> f64 {
        if self.is_done.load(Acquire) {
            return 1.0;
        }
        self.completed.load(Acquire) as f64 / self.total as f64
    }

    async fn try_apply_job(&self) -> bool {
        if self.is_done.load(Acquire) || self.is_canceled.load(Acquire) {
            return false;
        }

        // if set the maximum rate, need to check whether it is currently
        // possible to send the request
        if let Some(limiter) = &self.limiter {
            limiter.allow().await;
        }

        // is there any chance of apply a job
        if self.applied.load(Acquire) < self.total {
            let previous = self.applied.fetch_add(1, SeqCst);
            if previous >= self.total {
                return false;
            }
        } else {
            return false;
        }

        true
    }

    fn complete_job(&self) {
        self.completed.fetch_add(1, SeqCst);
        if self.completed.load(Acquire) >= self.total
            && !self.is_done.load(Acquire)
        {
            self.is_done.store(true, SeqCst);
        }
    }

    fn cancel(&mut self) {
        if !self.is_canceled.load(Acquire) {
            self.is_canceled.store(true, SeqCst);
        }
    }
}

/// [DurationDispatcher] is a duration-based task dispatcher
pub struct DurationDispatcher {
    /// the number of requests executed
    total: AtomicU64,

    /// start time for executing test
    start: Instant,

    /// total duration for execute test
    duration: Duration,

    /// a rate limiter that limits the acquisition of a fixed number of tokens
    /// per second
    limiter: Option<Limiter>,

    /// indicates whether it is canceled
    is_canceled: AtomicBool,

    /// cancel time
    canceled_at: Option<Instant>,

    /// indicate whether to complete
    is_done: AtomicBool,
}

impl DurationDispatcher {
    pub fn new(duration: Duration, rate: &Option<u16>) -> Self {
        Self {
            duration,
            canceled_at: None,
            start: Instant::now(),
            limiter: new_limiter(rate),
            total: AtomicU64::new(0),
            is_canceled: AtomicBool::new(false),
            is_done: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl Dispatcher for DurationDispatcher {
    fn get_process(&self) -> f64 {
        if self.is_done.load(Acquire) {
            return 1.0;
        }

        if self.is_canceled.load(Acquire) {
            if let Some(canceled_at) = self.canceled_at {
                let run_time = canceled_at - self.start;
                return run_time.as_secs() as f64
                    / self.duration.as_secs() as f64;
            }
        }

        let run_time = Instant::now() - self.start;
        run_time.as_secs() as f64 / self.duration.as_secs() as f64
    }

    async fn try_apply_job(&self) -> bool {
        if self.is_done.load(Acquire) || self.is_canceled.load(Acquire) {
            return false;
        }

        // if set the maximum rate, need to check whether it is currently
        // possible to send the request
        if let Some(limiter) = &self.limiter {
            limiter.allow().await;
        }

        // when get the token, the time has expired, return and exit
        if Instant::now() - self.start >= self.duration {
            return false;
        }

        self.total.fetch_add(1, SeqCst);
        true
    }

    fn complete_job(&self) {
        if Instant::now() - self.start >= self.duration
            && !self.is_done.load(Acquire)
        {
            self.is_done.store(true, SeqCst);
        }
    }

    fn cancel(&mut self) {
        if !self.is_canceled.load(Acquire) {
            self.is_canceled.store(true, SeqCst);
            self.canceled_at = Some(Instant::now());
        }
    }
}
