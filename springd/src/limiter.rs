use governor::{
    clock::DefaultClock,
    state::{direct::NotKeyed, InMemoryState},
    Quota, RateLimiter,
};
use std::num::NonZeroU32;
use tokio::time;

/// Limiter limit only sending a fixed number of requests per second
pub(crate) struct Limiter {
    inner: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
}

impl Limiter {
    /// create a new Limiter
    pub fn new(rate: u16) -> Limiter {
        Self {
            inner: RateLimiter::direct(Quota::per_second(
                NonZeroU32::new(rate as u32).unwrap(),
            )),
        }
    }

    /// allow function return means that the next action can be performed,
    /// otherwise wait here
    pub(crate) async fn allow(&self) {
        loop {
            let result = self.inner.check();
            if result.is_ok() {
                break;
            }
            time::sleep(time::Duration::from_nanos(100)).await;
        }
    }
}
