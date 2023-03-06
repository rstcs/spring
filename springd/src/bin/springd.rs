use clap::{crate_authors, Parser};
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
struct Args {
    #[arg(
        long,
        short,
        default_value_t = 125,
        help = "Maximum number of concurrent connections"
    )]
    connections: u16,

    #[arg(
        long,
        short,
        value_parser = parse_duration,
        default_value = "30s",
        help = "Socket/request timeout"
    )]
    timeout: Duration,

    #[arg(long, short, help = "Print latency statistics")]
    latencies: bool,

    #[arg(long, short, default_value = "GET", help = "Request method")]
    method: String,

    #[arg(long, short, help = "Request Body")]
    body: Option<String>,

    #[arg(long, short = 'f', help = "File to use as Request Body")]
    body_file: Option<PathBuf>,

    #[arg(long, help = "Path to the client's TLS Certificate")]
    cert: Option<PathBuf>,

    #[arg(long, help = "Path to the client's TLS Certificate Private Key")]
    key: Option<PathBuf>,

    #[arg(
        long,
        short = 'k',
        help = "Controls whether a client verifies the server's certificate chain and host name"
    )]
    insecure: bool,

    #[arg(long, short = 'a', help = "Disable HTTP keep-alive")]
    disable_keep_alive: bool,

    #[arg(
        long,
        short = 'H',
        num_args = 0..,
        help = "HTTP headers to use(can be repeated)",
        value_delimiter = ' '
    )]
    headers: Vec<String>,

    #[arg(long, short = 'n', help = "Number of requests", value_delimiter = ' ')]
    requests: Option<u64>,

    #[arg(
        long,
        short = 'd',
        value_parser = parse_duration,
        help = "Duration of test"
    )]
    duration: Option<Duration>,
}

fn main() {
    let args = Args::parse();
    println!("{:#?}, {:?}", args, Duration::from_secs(333));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_number() {
        assert_eq!(is_number("12234"), true);
        assert_eq!(is_number("12234s"), false);
    }
}
