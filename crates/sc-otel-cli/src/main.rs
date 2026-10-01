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
    if let Ok(exit) = std::panic::catch_unwind(AssertUnwindSafe(run)) {
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
            let _ = error.print();
            return exit;
        }
    };
    run::run(&cli)
}
