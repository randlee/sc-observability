//! `sc-otel` sends OpenTelemetry logs, spans and metrics over OTLP/HTTP.

mod cli;
#[cfg(test)]
mod cli_docs;
mod constants;
mod error_codes;
mod send;

use clap::Parser;
use sc_observability_otlp::{error_codes::TELEMETRY_EXPORT_FAILED, sync::SyncError};
use std::{panic::AssertUnwindSafe, process::ExitCode};

fn main() -> ExitCode {
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(AssertUnwindSafe(run));
    std::panic::set_hook(previous_hook);
    match result {
        Ok(exit) => ExitCode::from(exit),
        Err(payload) => {
            let include_payload =
                panic_details_enabled(std::env::var(constants::PANIC_DETAILS_ENV).ok().as_deref());
            eprintln!("{}", panic_diagnostic(&*payload, include_payload));
            ExitCode::from(error_codes::EXIT_INTERNAL)
        }
    }
}

fn panic_details_enabled(setting: Option<&str>) -> bool {
    setting == Some("1")
}

fn panic_diagnostic(payload: &(dyn std::any::Any + Send), include_payload: bool) -> String {
    let mut diagnostic = format!(
        "sc-otel: unexpected internal error [{}].\nRecovery: Retry the command; if the error persists, report this code and the sc-otel version. Set SC_OTEL_DEBUG_PANIC=1 to include local diagnostic details.",
        constants::INTERNAL_ERROR_CODE
    );
    if include_payload {
        diagnostic.push_str("\nDiagnostic: panic payload (shown because SC_OTEL_DEBUG_PANIC=1): ");
        diagnostic.push_str(panic_message(payload));
    }
    diagnostic
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
            let exit = if error.use_stderr() {
                error_codes::EXIT_USAGE
            } else {
                error_codes::EXIT_OK
            };
            if let Err(print_error) = error.print() {
                eprintln!("sc-otel: unable to render usage error: {print_error}");
            }
            return exit;
        }
    };
    match send::run(&cli) {
        Ok(()) => error_codes::EXIT_OK,
        Err(error) => {
            // Display only: the client redacts header values and URL userinfo there.
            match &error {
                SyncError::Export(_) => eprintln!(
                    "sc-otel: {TELEMETRY_EXPORT_FAILED}: {error}; \
                     check the endpoint and --root-certificate; raise --timeout if needed"
                ),
                SyncError::Validation { .. } => eprintln!("sc-otel: {error}"),
            }
            send::exit_code(&error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{panic_details_enabled, panic_diagnostic};

    #[test]
    fn panic_payload_requires_the_exact_opt_in_value() {
        assert!(!panic_details_enabled(None));
        assert!(!panic_details_enabled(Some("0")));
        assert!(!panic_details_enabled(Some("true")));
        assert!(panic_details_enabled(Some("1")));
    }

    #[test]
    fn panic_diagnostic_hides_payload_by_default_and_gives_recovery_guidance() {
        let diagnostic = panic_diagnostic(&"Authorization: Bearer secret", false);

        assert!(
            diagnostic.starts_with("sc-otel: unexpected internal error [SC_OTEL_CLI_INTERNAL].")
        );
        assert!(diagnostic.contains(
            "Recovery: Retry the command; if the error persists, report this code and the sc-otel version."
        ));
        assert!(
            diagnostic.contains("Set SC_OTEL_DEBUG_PANIC=1 to include local diagnostic details.")
        );
        assert!(!diagnostic.contains("Authorization"));
        assert!(!diagnostic.contains("secret"));
    }

    #[test]
    fn panic_diagnostic_shows_payload_only_when_explicitly_enabled() {
        let diagnostic = panic_diagnostic(&"Authorization: Bearer secret", true);

        assert!(diagnostic.contains("SC_OTEL_DEBUG_PANIC=1"));
        assert!(diagnostic.contains("Authorization: Bearer secret"));
    }
}
