use std::error::Error as StdError;

pub type Error = Box<dyn StdError + Send + Sync + 'static>;

pub type Result<T> = std::result::Result<T, Error>;
