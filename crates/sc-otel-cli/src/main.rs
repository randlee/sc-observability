//! `sc-otel` submits and inspects durable telemetry envelopes.

mod cli;
mod client;
mod config;
mod constants;
mod error;
mod exit;
mod input;
mod output;
mod run;

use clap::Parser;
use std::{panic::AssertUnwindSafe, process::ExitCode};

fn main() -> ExitCode {
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(AssertUnwindSafe(run));
    std::panic::set_hook(previous_hook);
    if let Ok(exit) = result {
        ExitCode::from(exit)
    } else {
        eprintln!("sc-otel: unexpected internal error");
        ExitCode::from(constants::EXIT_INTERNAL)
    }
}

fn run() -> u8 {
    let cli = match cli::Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let exit = if error.use_stderr() {
                constants::EXIT_USAGE
            } else {
                constants::EXIT_OK
            };
            if let Err(print_error) = error.print() {
                eprintln!("sc-otel: unable to render usage error: {print_error}");
            }
            return exit;
        }
    };
    run::run(&cli)
}
