//! `sc-otel` submits and inspects durable telemetry envelopes.

mod cli;
mod client;
mod config;
mod constants;
mod error;
mod error_codes;
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
    match result {
        Ok(exit) => ExitCode::from(exit),
        Err(payload) => {
            eprintln!(
                "sc-otel: unexpected internal error: {}",
                panic_message(&*payload)
            );
            ExitCode::from(constants::EXIT_INTERNAL)
        }
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("non-string panic payload")
}

fn run() -> u8 {
    let cli = match cli::Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let exit = exit::parser_exit(&error);
            if let Err(print_error) = error.print() {
                eprintln!("sc-otel: unable to render usage error: {print_error}");
            }
            return exit;
        }
    };
    run::run(&cli)
}

#[cfg(test)]
mod tests {
    use super::panic_message;

    #[test]
    fn panic_message_preserves_string_payloads_and_bounds_other_payloads() {
        assert_eq!(panic_message(&"expected panic"), "expected panic");
        assert_eq!(panic_message(&String::from("owned panic")), "owned panic");
        assert_eq!(panic_message(&42_u8), "non-string panic payload");
    }
}
