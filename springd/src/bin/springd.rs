use clap::{CommandFactory, Parser};
use clap_complete::generate;
use indicatif::{ProgressBar, ProgressStyle};
use springd::{Arg, Task};
use std::io;
use std::sync::Arc;

fn create_count_progress_bar(arg: &Arg) -> ProgressBar {
    let pb = ProgressBar::new(arg.requests.unwrap());
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} \
            ({per_sec}, {percent}%, {eta})",
        )
        .unwrap()
        .progress_chars("#>-"),
    );
    pb
}

fn create_duration_progress_bar(arg: &Arg) -> ProgressBar {
    let pb = ProgressBar::new(arg.duration.unwrap().as_secs());
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}s/{len}s",
        )
        .unwrap()
        .progress_chars("#>-"),
    );
    pb
}

fn create_progress_bar(arg: &Arg) -> ProgressBar {
    if let Some(_) = arg.requests {
        create_count_progress_bar(arg)
    } else {
        create_duration_progress_bar(arg)
    }
}

fn print_tip(arg: &Arg) {
    if arg.requests.is_some() {
        println!(
            "  {:?} {:?} with {} requests using {} connections",
            arg.method,
            arg.url.clone().unwrap(),
            arg.requests.unwrap(),
            arg.connections
        );
    } else if arg.duration.is_some() {
        println!(
            "  {:?} {:?} with for {:?} using {} connections",
            arg.method,
            arg.url.clone().unwrap(),
            arg.duration.unwrap(),
            arg.connections
        );
    }
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let arg = Arg::parse();

    if let Some(shell) = arg.completions {
        let mut cmd = Arg::command();
        let app_name = cmd.get_name().to_string();
        generate(shell, &mut cmd, app_name, &mut io::stdout());
        std::process::exit(0);
    }

    print_tip(&arg);
    let pb = create_progress_bar(&arg);
    Arc::new(Task::new(arg, Some(pb))?).run()
}
