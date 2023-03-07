use clap::{CommandFactory, Parser};
use clap_complete::generate;
use springd::Arg;
use std::io;

fn main() {
    let arg = Arg::parse();

    if let Some(shell) = arg.completions {
        let mut cmd = Arg::command();
        let app_name = cmd.get_name().to_string();
        generate(shell, &mut cmd, app_name, &mut io::stdout());
        std::process::exit(0);
    }

    let cmd = Arg::command();

    println!("green, true, {:#?}, {}", arg, cmd.get_color());
}
