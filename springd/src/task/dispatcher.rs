//! dispatcher module is used to distribute tasks according to different models

use async_trait::async_trait;
use governor::{
    clock::DefaultClock,
    state::{direct::NotKeyed, InMemoryState},
    Quota, RateLimiter,
};
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::*};
use tokio::time;

type Limiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

#[async_trait]
pub trait Dispatcher: Send + Sync {
    /// query current task process, returning 0 to 1
    fn get_process(&self) -> f64;

    /// worker apply a job from dispatcher, return true continue to handle,
    /// return false worker will exit.
    async fn try_apply_job(&self) -> bool;

    /// when worker complete job, it will notify the dispatcher
    fn complete_job(&mut self);

    /// when the program receives an external termination signal, notify the
    /// Dispatcher to process it
    fn cancel(&mut self);
}

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

impl CountDispatcher {
    pub fn new(total: u64, rate: &Option<u16>) -> Self {
        let mut limiter: Option<Limiter> = None;
        if let Some(rate) = rate {
            limiter = Some(RateLimiter::direct(Quota::per_second(
                NonZeroU32::new(*rate as u32).unwrap(),
            )));
        }
        Self {
            limiter,
            total,
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
        self.completed.load(Acquire) as f64 / self.total as f64
    }

    async fn try_apply_job(&self) -> bool {
        if self.is_done.load(Acquire) || self.is_canceled.load(Acquire) {
            return false;
        }

        // is there any chance of apply a job
        if self.applied.load(Acquire) < self.total {
            let previous = self.applied.fetch_add(1, SeqCst);
            if previous >= self.total {
                return false;
            }
        }

        // if set the maximum rate, need to check whether it is currently
        // possible to send the request
        if let Some(limiter) = &self.limiter {
            loop {
                let result = limiter.check();
                if result.is_ok() {
                    break;
                }
                time::sleep(time::Duration::from_nanos(100)).await;
            }
        }

        true
    }

    fn complete_job(&mut self) {
        let _ = self.completed.fetch_add(1, SeqCst);
        if self.completed.load(SeqCst) >= self.total
            && !self.is_done.load(SeqCst)
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
