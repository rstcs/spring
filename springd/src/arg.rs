//! arg module define the application entry arguments [Arg]

use clap::{
    builder::{
        IntoResettable, OsStr, PossibleValue,
        Resettable::{self, *},
    },
    Parser, ValueEnum, ValueHint,
};
use clap_complete::Shell;
use std::path::PathBuf;
use std::time::Duration;

fn is_number(s: &str) -> bool {
    s.parse::<u64>().is_ok()
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

/// define supported http methods
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Head,
    Patch,
}

impl Method {
    /// convert to [reqwest::Method]
    pub(crate) fn to_reqwest_method(&self) -> reqwest::Method {
        match self {
            Method::Get => reqwest::Method::GET,
            Method::Post => reqwest::Method::POST,
            Method::Put => reqwest::Method::PUT,
            Method::Patch => reqwest::Method::PATCH,
            Method::Delete => reqwest::Method::DELETE,
            Method::Head => reqwest::Method::HEAD,
        }
    }
}

impl IntoResettable<OsStr> for Method {
    fn into_resettable(self) -> Resettable<OsStr> {
        match self {
            Method::Get => Value(OsStr::from("GET")),
            Method::Post => Value(OsStr::from("POST")),
            Method::Put => Value(OsStr::from("PUT")),
            Method::Delete => Value(OsStr::from("DELETE")),
            Method::Head => Value(OsStr::from("HEAD")),
            Method::Patch => Value(OsStr::from("PATCH")),
        }
    }
}

impl ValueEnum for Method {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Method::Get,
            Method::Put,
            Method::Post,
            Method::Delete,
            Method::Head,
            Method::Patch,
        ]
    }

    fn to_possible_value<'a>(&self) -> Option<PossibleValue> {
        Some(match self {
            Method::Get => PossibleValue::new("GET"),
            Method::Put => PossibleValue::new("PUT"),
            Method::Post => PossibleValue::new("POST"),
            Method::Delete => PossibleValue::new("DELETE"),
            Method::Head => PossibleValue::new("HEAD"),
            Method::Patch => PossibleValue::new("PATCH"),
        })
    }
}

#[derive(Debug, Parser)]
#[command(author, version, about, allow_missing_positional(true))]
#[command(help_template(
    "\
{before-help}{name}({version}){tab}{about-with-newline}
{usage-heading} {usage}

{all-args}{after-help}\
"
))]
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
    #[arg(
        long,
        short,
        default_value = Method::Get,
        value_enum,
        help = "Request method"
    )]
    pub(crate) method: Method,

    /// Request Body
    #[arg(long, short, conflicts_with = "body_file", help = "Request Body")]
    pub(crate) body: Option<String>,

    /// File to use as Request Body
    #[arg(
        long,
        short = 'f',
        value_hint = ValueHint::FilePath,
        conflicts_with = "body",
        help = "File to use as Request Body"
    )]
    pub(crate) body_file: Option<PathBuf>,

    /// Path to the client's TLS Certificate
    #[arg(
        long,
        value_hint = ValueHint::FilePath,
        requires("key"),
        help = "Path to the client's TLS Certificate"
    )]
    pub(crate) cert: Option<PathBuf>,

    /// Path to the client's TLS Certificate Private Key
    #[arg(
        long,
        requires("cert"),
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
    pub(crate) url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    const URI: &str = "https://localhost/test";

    #[test]
    fn test_is_number() {
        assert!(is_number("12234"));
        assert!(!is_number("12234s"));
    }

    #[test]
    fn test_parse_duration() {
        assert!(parse_duration("123").is_ok());
        assert!(parse_duration("123s").is_ok());
        assert!(parse_duration("123x").is_err());
    }

    #[test]
    fn test_method_choices() {
        let mut cmd = Arg::command();
        let methods = vec!["GET", "PUT", "POST", "DELETE", "HEAD", "PATCH"];
        for method in methods {
            let args = vec!["springd", "-n", "20", "-m", method, URI];
            let result = cmd.try_get_matches_from_mut(args);
            assert!(result.is_ok());
        }

        let result = cmd.try_get_matches_from_mut(vec![
            "springd", "-n", "20", "-m", "get", URI,
        ]);
        assert!(result.as_ref().is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg
            .contains("possible values: GET, PUT, POST, DELETE, HEAD, PATCH"));
    }

    #[test]
    fn test_required_parameters() {
        let mut cmd = Arg::command();
        let result = cmd.try_get_matches_from_mut(vec!["springd"]);
        assert!(result.as_ref().is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains(
            "error: the following required arguments were not provided:
  --requests <REQUESTS>
  --duration <DURATION>
  <URI>"
        ))
    }

    #[test]
    fn test_must_provide_requests_or_duration_parameters() {
        let mut cmd = Arg::command();

        // neither provided
        let result = cmd.try_get_matches_from_mut(vec!["springd", URI]);
        assert!(result.as_ref().is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains(
            "error: the following required arguments were not provided:
  --requests <REQUESTS>
  --duration <DURATION>"
        ));

        // both provide
        let result = cmd.try_get_matches_from_mut(vec![
            "springd", "-n", "20", "-d", "300", URI,
        ]);
        assert!(result.as_ref().is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains(
            "error: the argument '--requests <REQUESTS>' \
        cannot be used with '--duration <DURATION>'"
        ));
    }

    #[test]
    fn test_require_key_and_cert_at_the_same_time() {
        let mut cmd = Arg::command();

        // only provide key
        let result = cmd.try_get_matches_from_mut(vec![
            "springd", "-n", "20", "--key", "key.crt", URI,
        ]);
        assert!(result.as_ref().is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains(
            "error: the following required arguments were not provided:
  --cert <CERT>"
        ));

        // only provide cert
        let result = cmd.try_get_matches_from_mut(vec![
            "springd", "-n", "20", "--cert", "cert.crt", URI,
        ]);
        assert!(result.as_ref().is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains(
            "error: the following required arguments were not provided:
  --key <KEY>"
        ));

        // both provided
        let result = cmd.try_get_matches_from_mut(vec![
            "springd", "-n", "20", "--cert", "cert.crt", "--key", "key.crt",
            URI,
        ]);
        assert!(result.as_ref().is_ok());
    }
}
