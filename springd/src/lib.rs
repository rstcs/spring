#![feature(is_sorted)]
//! springd is a http server benchmark tool.
#![cfg_attr(feature = "cargo-clippy", allow(clippy::single_char_pattern))]
#![allow(dead_code, unused_mut)]

pub mod arg;
pub mod dispatcher;
pub mod limiter;
pub mod statistics;
pub mod task;

pub use self::arg::Arg;
pub use self::task::Task;
