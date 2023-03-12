use clap::{CommandFactory, Parser};
use clap_complete::generate;
use springd::{Arg, Statistics, Task};
use std::io;
use std::sync::Arc;

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let arg = Arg::parse();

    if let Some(shell) = arg.completions {
        let mut cmd = Arg::command();
        let app_name = cmd.get_name().to_string();
        generate(shell, &mut cmd, app_name, &mut io::stdout());
        std::process::exit(0);
    }

    Arc::new(Task::new(arg)?).run()
}
