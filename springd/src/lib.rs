//! springd is a http server benchmark tool.
#![cfg_attr(feature = "cargo-clippy", allow(clippy::single_char_pattern))]

pub mod arg;
pub mod task;

pub use self::arg::Arg;
