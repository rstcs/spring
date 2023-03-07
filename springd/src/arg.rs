//! arg module define the application entry arguments [Arg]

use clap::{crate_authors, Parser, ValueHint};
use clap_complete::Shell;
use std::path::PathBuf;
use std::time::Duration;

fn is_number(s: &str) -> bool {
    match s.parse::<u64>() {
        Ok(_) => true,
        Err(_) => false,
    }
}

fn parse_duration(arg: &str) -> Result<Duration, std::num::ParseIntError> {
    if is_number(arg) {
        return Ok(Duration::from_secs(arg.parse()?));
    }

    let mut input = arg;
    if input.ends_with("s") {
        input = &arg[..arg.len() - 1]
    }

    let seconds = input.parse()?;
    Ok(Duration::from_secs(seconds))
}

#[derive(Debug, Parser)]
#[command(author(crate_authors!("\n")), version, about, allow_missing_positional(true))]
pub struct Arg {
    /// Maximum number of concurrent connections
    #[arg(
        long,
        short,
        default_value_t = 125,
        help = "Maximum number of concurrent connections"
    )]
    pub(crate) connections: u16,

    /// Socket/request timeout
    #[arg(
        long,
        short,
        value_parser = parse_duration,
        default_value = "30s",
        help = "Socket/request timeout"
    )]
    pub(crate) timeout: Duration,

    /// Print latency statistics
    #[arg(long, short, help = "Print latency statistics")]
    pub(crate) latencies: bool,

    /// Request method
    #[arg(long, short, default_value = "GET", help = "Request method")]
    pub(crate) method: String,

    /// Request Body
    #[arg(long, short, help = "Request Body")]
    pub(crate) body: Option<String>,

    /// File to use as Request Body
    #[arg(
        long,
        short = 'f',
        value_hint = ValueHint::FilePath,
        help = "File to use as Request Body"
    )]
    pub(crate) body_file: Option<PathBuf>,

    /// Path to the client's TLS Certificate
    #[arg(
        long,
        value_hint = ValueHint::FilePath,
        help = "Path to the client's TLS Certificate"
    )]
    pub(crate) cert: Option<PathBuf>,

    /// Path to the client's TLS Certificate Private Key
    #[arg(
        long,
        value_hint = ValueHint::FilePath,
        help = "Path to the client's TLS Certificate Private Key"
    )]
    pub(crate) key: Option<PathBuf>,

    /// Controls whether a client verifies the server's
    /// certificate chain and host name
    #[arg(
        long,
        short = 'k',
        help = "Controls whether a client verifies the server's certificate chain and host name"
    )]
    pub(crate) insecure: bool,

    /// Disable HTTP keep-alive
    #[arg(long, short = 'a', help = "Disable HTTP keep-alive")]
    pub(crate) disable_keep_alive: bool,

    #[arg(
        long,
        short = 'H',
        num_args = 0..,
        help = "HTTP headers to use(can be repeated)",
        value_delimiter = ' '
    )]
    pub(crate) headers: Vec<String>,

    /// Number of requests
    #[arg(
        long,
        short = 'n',
        help = "Number of requests",
        value_delimiter = ' ',
        conflicts_with = "duration",
        required_unless_present_any(["duration", "completions"])
    )]
    pub(crate) requests: Option<u64>,

    /// Duration of test
    #[arg(
        long,
        short = 'd',
        value_parser = parse_duration,
        help = "Duration of test",
        conflicts_with = "requests",
        required_unless_present_any(["requests", "completions"])
    )]
    pub(crate) duration: Option<Duration>,

    /// Rate limit in requests per second
    #[arg(long, short = 'r', help = "Rate limit in requests per second")]
    pub(crate) rate: Option<u16>,

    #[arg(long, value_enum)]
    pub completions: Option<Shell>,

    /// Target Url
    #[arg(
        required_unless_present("completions"), 
        value_hint = ValueHint::Url,
        help = "Target Url"
    )]
    pub(crate) uri: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_number() {
        assert_eq!(is_number("12234"), true);
        assert_eq!(is_number("12234s"), false);
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("123").is_ok(), true);
        assert_eq!(parse_duration("123s").is_ok(), true);
        assert_eq!(parse_duration("123x").is_ok(), false);
    }
}
