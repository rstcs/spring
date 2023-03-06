use clap::Parser;
use std::time::Duration;

fn parse_duration(arg: &str) -> Result<std::time::Duration, std::num::ParseIntError> {
    if arg.ends_with("s") {}

    let seconds = arg.parse()?;
    Ok(std::time::Duration::from_secs(seconds))
}

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(
        long,
        short,
        default_value_t = 125,
        help = "Maximum number of concurrent connections"
    )]
    connections: u16,
    #[arg(long, short, value_parser = parse_duration, help = "Socket/request timeout", default_value = "30")]
    timeout: Duration,
}

fn main() {
    let args = Args::parse();

    // for _ in 0..args.count {
    //     println!("Hello {}!", args.name)
    // }
}
