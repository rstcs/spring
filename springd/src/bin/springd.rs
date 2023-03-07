use clap::Parser;
use springd::Arg;

fn main() {
    let arg = Arg::parse();
    println!("{:#?}", arg);
}
