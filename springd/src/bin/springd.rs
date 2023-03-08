use clap::{CommandFactory, Parser};
use clap_complete::generate;
use log::debug;
use springd::Arg;
use std::io;

fn main() {
    env_logger::init();
    let arg = Arg::parse();

    if let Some(shell) = arg.completions {
        let mut cmd = Arg::command();
        let app_name = cmd.get_name().to_string();
        generate(shell, &mut cmd, app_name, &mut io::stdout());
        std::process::exit(0);
    }

    let cmd = Arg::command();

    debug!("output {:#?}, {}", arg, cmd.get_color());
}
