#![feature(is_sorted)]
//! springd is a http server benchmark tool.
#![cfg_attr(feature = "cargo-clippy", allow(clippy::single_char_pattern))]
#![allow(dead_code, unused_mut)]

pub mod arg;
pub(crate) mod client;
pub(crate) mod dispatcher;
pub(crate) mod limiter;
pub(crate) mod request;
pub(crate) mod statistics;
pub mod task;

pub use self::arg::Arg;
pub use self::task::Task;
