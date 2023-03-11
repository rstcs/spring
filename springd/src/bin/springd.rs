use clap::{CommandFactory, Parser};
use clap_complete::generate;
use log::{debug, error, info};
use springd::task::Task;
use springd::Arg;
use std::io;
use std::sync::Arc;

fn main() {
    env_logger::init();
    let arg = Arg::parse();

    if let Some(shell) = arg.completions {
        let mut cmd = Arg::command();
        let app_name = cmd.get_name().to_string();
        generate(shell, &mut cmd, app_name, &mut io::stdout());
        std::process::exit(0);
    }

    let task =
        Task::new(arg).expect("programing exception, create task failed");
    let task = Arc::new(task);
    let result = task.clone().run();
    info!("process run success: {}", result.is_ok());
}
