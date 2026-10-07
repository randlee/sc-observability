//! trybuild compile-fail cases: one per row of the rejected-forms tables in
//! `docs/compatibility.md` (event macros and `#[instrument]`), plus the
//! `Entered`-across-`.await` `!Send` case, each with its expected stderr checked in.
//! Does not install the logger.
//!
//! Privacy cases live in `tests/compile_privacy/` instead: each is compiled as
//! the `src/main.rs` of an isolated consumer crate and judged from rustc's
//! structured diagnostics (error code and primary consumer span), never from
//! rendered text that names crate-private definitions.
#![cfg(feature = "v1")]
#![allow(
    deprecated,
    clippy::expect_used,
    clippy::panic,
    reason = "integration test: helper fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

#[test]
fn rejected_event_forms_fail_to_compile() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}

/// R-A4-005: a `LogControl` (and so its operations) exists only through
/// `LogGuard::control`; forging one fails with E0624 at the consumer call.
#[test]
fn log_control_is_not_constructible_outside_the_crate() {
    let probe = ConsumerProbe::new("log-control-privacy-probe");

    // Positive control: the dependency builds and its public `LogControl` is
    // nameable, so the failure below can only be the privacy check.
    let control =
        probe.check("fn _named(_: Option<sc_observability_log::LogControl>) {}\nfn main() {}\n");
    assert!(
        control.success && control.errors.is_empty(),
        "positive control did not build; this is an infrastructure failure, not privacy evidence\n{control}"
    );

    let fixture = include_str!("compile_privacy/log_control_not_constructible.rs");
    let forged = probe.check(fixture);
    assert!(!forged.success, "forging LogControl compiled\n{forged}");
    let [error] = forged.errors.as_slice() else {
        panic!("expected exactly one coded consumer error\n{forged}");
    };
    let call_line = fixture
        .lines()
        .position(|line| line.contains("LogControl::new()"))
        .map(|index| index + 1)
        .expect("fixture calls LogControl::new()");
    assert_eq!(error.code, "E0624", "{forged}");
    assert_eq!(error.file, "src/main.rs", "{forged}");
    assert_eq!(error.line, call_line, "{forged}");
    assert_eq!(error.highlighted, "new", "{forged}");
}

/// A consumer crate outside this workspace with a path dependency on this
/// crate and its own target directory.
struct ConsumerProbe {
    root: tempfile::TempDir,
    target_dir: PathBuf,
    package: &'static str,
}

impl ConsumerProbe {
    fn new(package: &'static str) -> Self {
        let root = tempfile::tempdir().expect("probe directory");
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        std::fs::write(
            root.path().join("Cargo.toml"),
            format!(
                "[package]\nname = \"{package}\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
                 [dependencies]\nsc-observability-log = {{ path = {:?} }}\n\n[workspace]\n",
                crate_dir.display().to_string()
            ),
        )
        .expect("probe manifest");
        // Start from the workspace lock so resolution reuses its pinned versions.
        let lock = crate_dir.join("../../Cargo.lock");
        if lock.is_file() {
            std::fs::copy(&lock, root.path().join("Cargo.lock")).expect("probe lockfile");
        }
        std::fs::create_dir(root.path().join("src")).expect("probe src");
        let target_dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(package);
        Self {
            root,
            target_dir,
            package,
        }
    }

    fn check(&self, main_rs: &str) -> CheckOutcome {
        std::fs::write(self.root.path().join("src/main.rs"), main_rs).expect("probe main.rs");
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let output = Command::new(cargo)
            .args(["check", "--quiet", "--message-format=json"])
            .current_dir(self.root.path())
            .env("CARGO_TARGET_DIR", &self.target_dir)
            .output()
            .expect("run cargo check for the probe");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let mut errors = Vec::new();
        let mut foreign = Vec::new();
        let mut consumer_messages = 0_usize;
        let mut rendered = String::new();
        for line in stdout.lines() {
            let Ok(message) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if message["reason"] != "compiler-message" {
                continue;
            }
            let diagnostic = &message["message"];
            if diagnostic["level"] != "error" {
                continue;
            }
            // The probe's only target is its binary, named after the package.
            if message["target"]["name"] != self.package {
                foreign.push(
                    diagnostic["rendered"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                );
                continue;
            }
            consumer_messages += 1;
            rendered.push_str(diagnostic["rendered"].as_str().unwrap_or_default());
            // Only coded errors count; the uncoded "aborting due to" summary does not.
            let Some(code) = diagnostic["code"]["code"].as_str() else {
                continue;
            };
            let primary = diagnostic["spans"]
                .as_array()
                .and_then(|spans| spans.iter().find(|span| span["is_primary"] == true))
                .expect("coded error has a primary span");
            let text = &primary["text"][0];
            let column = |key: &str| {
                text[key]
                    .as_u64()
                    .and_then(|value| usize::try_from(value).ok())
                    .unwrap_or(1)
            };
            let (start, end) = (column("highlight_start"), column("highlight_end"));
            errors.push(ConsumerError {
                code: code.to_owned(),
                // rustc reports the path with the host separator.
                file: primary["file_name"]
                    .as_str()
                    .unwrap_or_default()
                    .replace('\\', "/"),
                line: primary["line_start"]
                    .as_u64()
                    .and_then(|value| usize::try_from(value).ok())
                    .unwrap_or_default(),
                highlighted: text["text"]
                    .as_str()
                    .and_then(|source| source.get(start.saturating_sub(1)..end.saturating_sub(1)))
                    .unwrap_or_default()
                    .to_owned(),
            });
        }
        assert!(
            foreign.is_empty(),
            "a dependency failed to compile; infrastructure failure, not privacy evidence\n{}\n{stderr}",
            foreign.join("\n")
        );
        assert!(
            output.status.success() || consumer_messages > 0,
            "cargo failed before compiling the probe (resolution, network or toolchain); \
             infrastructure failure, not privacy evidence\n{stderr}"
        );
        CheckOutcome {
            success: output.status.success(),
            errors,
            rendered,
            stderr,
        }
    }
}

struct CheckOutcome {
    success: bool,
    errors: Vec<ConsumerError>,
    /// Rendered consumer diagnostics, shown only when an assertion fails.
    rendered: String,
    stderr: String,
}

impl std::fmt::Display for CheckOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "success: {}", self.success)?;
        for error in &self.errors {
            writeln!(
                f,
                "error[{}] {}:{} `{}`",
                error.code, error.file, error.line, error.highlighted
            )?;
        }
        write!(f, "{}{}", self.rendered, self.stderr)
    }
}

struct ConsumerError {
    code: String,
    file: String,
    line: usize,
    highlighted: String,
}
