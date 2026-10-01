//! Compile-time contracts for the public logger typestate API.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "integration test helpers surface compiler infrastructure failures"
)]

use std::path::{Path, PathBuf};
use std::process::Command;

struct ConsumerProbe {
    root: tempfile::TempDir,
    target_dir: PathBuf,
    package: &'static str,
}

impl ConsumerProbe {
    fn new(package: &'static str) -> Self {
        let root = tempfile::tempdir().expect("consumer project directory");
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        std::fs::write(
            root.path().join("Cargo.toml"),
            format!(
                "[package]\nname = \"{package}\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
                 [dependencies]\nsc-observability = {{ path = {:?} }}\n\n[workspace]\n",
                crate_dir.display().to_string()
            ),
        )
        .expect("consumer manifest");
        std::fs::copy(
            crate_dir.join("../../Cargo.lock"),
            root.path().join("Cargo.lock"),
        )
        .expect("consumer lockfile");
        std::fs::create_dir(root.path().join("src")).expect("consumer source directory");

        let target_dir = std::env::var_os("CARGO_TARGET_TMPDIR")
            .map_or_else(|| root.path().join("target"), PathBuf::from)
            .join(package);
        // Regression guard for the old `read_dir` implementation: an arbitrary
        // stale artifact must be inert because Cargo resolves the path dependency.
        let stale = target_dir.join("debug/deps/libsc_observability-stale.rlib");
        std::fs::create_dir_all(stale.parent().expect("stale artifact directory"))
            .expect("stale artifact directory");
        std::fs::write(stale, b"not a Cargo-selected artifact").expect("stale artifact");

        Self {
            root,
            target_dir,
            package,
        }
    }

    fn check(&self, source: &str) -> ConsumerCheck {
        std::fs::write(self.root.path().join("src/main.rs"), source).expect("consumer source");
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let output = Command::new(cargo)
            .args(["check", "--offline", "--quiet", "--message-format=json"])
            .current_dir(self.root.path())
            .env("CARGO_TARGET_DIR", &self.target_dir)
            .output()
            .expect("run released consumer compiler");

        let mut diagnostics = Vec::new();
        let mut foreign_errors = Vec::new();
        let mut consumer_errors = 0_usize;
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let Ok(message) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if message["reason"] != "compiler-message" || message["message"]["level"] != "error" {
                continue;
            }
            let diagnostic = &message["message"];
            if message["target"]["name"] != self.package {
                foreign_errors.push(
                    diagnostic["rendered"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                );
                continue;
            }
            consumer_errors += 1;
            if diagnostic
                .pointer("/code/code")
                .and_then(|code| code.as_str())
                == Some("deprecated")
            {
                diagnostics.push(diagnostic.clone());
            }
        }
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        assert!(
            foreign_errors.is_empty(),
            "dependency compilation failed; not consumer evidence\n{}\n{stderr}",
            foreign_errors.join("\n")
        );
        assert!(
            output.status.success() || consumer_errors > 0,
            "cargo failed before compiling the consumer\n{stderr}"
        );
        ConsumerCheck {
            success: output.status.success(),
            diagnostics,
            stderr,
        }
    }
}

struct ConsumerCheck {
    success: bool,
    diagnostics: Vec<serde_json::Value>,
    stderr: String,
}

struct DeprecatedConsumerDiagnostic {
    message: String,
    source: String,
}

fn deprecated_consumer_diagnostics(
    diagnostics: impl IntoIterator<Item = serde_json::Value>,
) -> Vec<DeprecatedConsumerDiagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| {
            let span = diagnostic["spans"]
                .as_array()
                .expect("deprecated diagnostic spans")
                .iter()
                .find(|span| span["is_primary"].as_bool() == Some(true))
                .expect("deprecated diagnostic primary span");
            assert!(
                span["file_name"]
                    .as_str()
                    .is_some_and(|path| path.replace('\\', "/") == "src/main.rs"),
                "deprecated diagnostics must point at the consumer: {diagnostic}"
            );
            let source = span["text"]
                .as_array()
                .and_then(|text| text.first())
                .and_then(|text| text["text"].as_str())
                .expect("deprecated diagnostic source text")
                .to_owned();
            let message = diagnostic["message"]
                .as_str()
                .expect("deprecated diagnostic message")
                .to_owned();
            DeprecatedConsumerDiagnostic { message, source }
        })
        .collect()
}

#[test]
fn logger_typestate_and_released_deprecation_contracts() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/logger_stopped_operations.rs");
    cases.pass("tests/ui/logger_running_to_stopped.rs");
    cases.compile_fail("tests/ui/v2_logger_stopped_operations.rs");
    let probe = ConsumerProbe::new("released-root-deprecation-consumer");
    let positive = probe.check(include_str!(
        "compile_diagnostics/released_nondeprecated_methods.rs"
    ));
    assert!(
        positive.success,
        "nondeprecated released methods failed to compile: {}",
        positive.stderr
    );

    let deprecated = probe.check(include_str!(
        "compile_diagnostics/root_deprecated_methods.rs"
    ));
    assert!(
        !deprecated.success,
        "deprecated consumer must fail: {}",
        deprecated.stderr
    );
    let deprecated = deprecated_consumer_diagnostics(deprecated.diagnostics);
    assert_eq!(deprecated.len(), 8, "unexpected deprecated diagnostics");
    for (method, note) in [
        (
            "LoggerBuilder::new",
            "Use LoggerBuilder::new_typed(); see migrate-error-api.md.",
        ),
        (
            "Logger::builder",
            "Use Logger::builder_typed(); see migrate-error-api.md.",
        ),
        (
            "Logger::new",
            "Use Logger::new_typed(); see migrate-error-api.md.",
        ),
        (
            "Logger::log",
            "Use Logger::log_typed(); see migrate-error-api.md.",
        ),
        (
            "Logger::try_log",
            "Use Logger::try_log_typed(); see migrate-error-api.md.",
        ),
        (
            "Logger::try_log_with_outcome",
            "Use Logger::try_log_with_outcome_typed(); see migrate-error-api.md.",
        ),
        (
            "Logger::emit",
            "Use log() for blocking queue admission or try_log() for non-blocking logging.",
        ),
        (
            "Logger::flush",
            "Use Logger::flush_typed(); see migrate-error-api.md.",
        ),
    ] {
        assert!(
            deprecated
                .iter()
                .any(|diagnostic| diagnostic.source.contains(method)
                    && diagnostic.message.contains(note)),
            "missing released diagnostic for {method}"
        );
    }
}
