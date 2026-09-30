//! External-consumer proof for the released open extension traits.
//!
//! Each case compiles a consumer crate outside this workspace with a path
//! dependency on this crate and judges rustc's structured diagnostics (error
//! code and primary consumer line), never rendered text. A 1.4.1 consumer that
//! implements every released trait with the root errors must build unchanged;
//! a canonical consumer implements the `v2` traits with the `v2` span and
//! metric models; and the negative control
//! proves the probe fails when root-error implementations meet `v2` traits.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "integration test: helper fns are not covered by clippy.toml allow-*-in-tests"
)]

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// A 1.4.1 consumer: implements every released open trait with the released
/// root errors and uses the released registrations and resolver policy.
const RELEASED_CONSUMER: &str = r"#![allow(deprecated)]
use std::sync::Arc;

use sc_observability_types::{
    LogEvent, LogProjector, MetricProjector, MetricRecord, Observation, ObservationSubscriber,
    ProcessIdentity, ProcessIdentityPolicy, ProcessIdentityResolver, ProjectionRegistration,
    SpanProjector, SpanSignal, SubscriberRegistration,
};

struct Ev;
struct Resolver;
impl ProcessIdentityResolver for Resolver {
    fn resolve(&self) -> Result<ProcessIdentity, sc_observability_types::IdentityError> {
        Ok(ProcessIdentity::default())
    }
}
struct Sub;
impl ObservationSubscriber<Ev> for Sub {
    fn observe(&self, _: &Observation<Ev>) -> Result<(), sc_observability_types::SubscriberError> {
        Ok(())
    }
}
struct Proj;
impl LogProjector<Ev> for Proj {
    fn project_logs(&self, _: &Observation<Ev>) -> Result<Vec<LogEvent>, sc_observability_types::ProjectionError> {
        Ok(Vec::new())
    }
}
impl SpanProjector<Ev> for Proj {
    fn project_spans(&self, _: &Observation<Ev>) -> Result<Vec<SpanSignal>, sc_observability_types::ProjectionError> {
        Ok(Vec::new())
    }
}
impl MetricProjector<Ev> for Proj {
    fn project_metrics(&self, _: &Observation<Ev>) -> Result<Vec<MetricRecord>, sc_observability_types::ProjectionError> {
        Ok(Vec::new())
    }
}

fn main() {
    let _policy = ProcessIdentityPolicy::Resolver(Arc::new(Resolver));
    let subscriber: Arc<dyn ObservationSubscriber<Ev>> = Arc::new(Sub);
    let (_subscriber, _filter) = SubscriberRegistration::new(subscriber).into_parts();
    let projector = Arc::new(Proj);
    let (_log, _span, _metric, _filter) = ProjectionRegistration::<Ev>::new()
        .with_log_projector(projector.clone())
        .with_span_projector(projector.clone())
        .with_metric_projector(projector)
        .into_parts();
}
";

/// A canonical consumer: implements the `v2` traits with the `v2` errors.
const CANONICAL_CONSUMER: &str = r"use std::sync::Arc;

use sc_observability_types::v2::{
    IdentityError, LogProjector, MetricProjector, MetricRecord, ObservationSubscriber,
    ProcessIdentityResolver, ProjectionError, ProjectionRegistration, SpanProjector, SpanSignal,
    SubscriberError, SubscriberRegistration,
};
use sc_observability_types::{LogEvent, Observation, ProcessIdentity, ProcessIdentityPolicy};

struct Ev;
struct Resolver;
impl ProcessIdentityResolver for Resolver {
    fn resolve(&self) -> Result<ProcessIdentity, IdentityError> {
        Ok(ProcessIdentity::default())
    }
}
struct Sub;
impl ObservationSubscriber<Ev> for Sub {
    fn observe(&self, _: &Observation<Ev>) -> Result<(), SubscriberError> {
        Ok(())
    }
}
struct Proj;
impl LogProjector<Ev> for Proj {
    fn project_logs(&self, _: &Observation<Ev>) -> Result<Vec<LogEvent>, ProjectionError> {
        Ok(Vec::new())
    }
}
impl SpanProjector<Ev> for Proj {
    fn project_spans(&self, _: &Observation<Ev>) -> Result<Vec<SpanSignal>, ProjectionError> {
        Ok(Vec::new())
    }
}
impl MetricProjector<Ev> for Proj {
    fn project_metrics(&self, _: &Observation<Ev>) -> Result<Vec<MetricRecord>, ProjectionError> {
        Ok(Vec::new())
    }
}

fn main() {
    let _policy = ProcessIdentityPolicy::v2_resolver(Arc::new(Resolver));
    let subscriber: Arc<dyn ObservationSubscriber<Ev>> = Arc::new(Sub);
    let (_subscriber, _filter) = SubscriberRegistration::new(subscriber).into_parts();
    let projector = Arc::new(Proj);
    let (_log, _span, _metric, _filter) = ProjectionRegistration::<Ev>::new()
        .with_log_projector(projector.clone())
        .with_span_projector(projector.clone())
        .with_metric_projector(projector)
        .into_parts();
}
";

/// The five released method signatures, in fixture order.
const RELEASED_METHODS: [&str; 5] = [
    "fn resolve(",
    "fn observe(",
    "fn project_logs(",
    "fn project_spans(",
    "fn project_metrics(",
];

#[test]
fn released_consumer_builds_unchanged() {
    let outcome = ConsumerProbe::new("released-trait-consumer").check(RELEASED_CONSUMER);
    assert!(
        outcome.success && outcome.errors.is_empty(),
        "an unchanged 1.4.1 consumer must build\n{outcome}"
    );
}

#[test]
fn canonical_consumer_builds() {
    let outcome = ConsumerProbe::new("canonical-trait-consumer").check(CANONICAL_CONSUMER);
    assert!(
        outcome.success && outcome.errors.is_empty(),
        "a canonical v2 consumer must build\n{outcome}"
    );
}

/// Negative control: the released implementations against the `v2` traits fail
/// with E0053 at each method, so the probe can observe the regression it guards.
#[test]
fn released_implementations_against_v2_traits_fail_with_e0053() {
    let fixture = RELEASED_CONSUMER.replacen(
        "use sc_observability_types::{\n    LogEvent, LogProjector, MetricProjector, MetricRecord, Observation, ObservationSubscriber,\n    ProcessIdentity, ProcessIdentityPolicy, ProcessIdentityResolver, ProjectionRegistration,\n    SpanProjector, SpanSignal, SubscriberRegistration,\n};",
        "use sc_observability_types::v2::{\n    LogProjector, MetricProjector, ObservationSubscriber, ProcessIdentityResolver,\n    ProjectionRegistration, SpanProjector, SubscriberRegistration,\n};\nuse sc_observability_types::{\n    LogEvent, MetricRecord, Observation, ProcessIdentity, SpanSignal,\n};",
        1,
    );
    assert_ne!(
        fixture, RELEASED_CONSUMER,
        "the control rewrote the imports"
    );
    // The released policy only accepts the released resolver; keep the control
    // about the five trait signatures.
    let fixture = fixture.replacen(
        "    let _policy = ProcessIdentityPolicy::Resolver(Arc::new(Resolver));\n",
        "",
        1,
    );
    let outcome = ConsumerProbe::new("released-trait-negative-control").check(&fixture);
    assert!(!outcome.success, "the negative control compiled\n{outcome}");
    let expected: Vec<usize> = RELEASED_METHODS
        .iter()
        .map(|method| {
            fixture
                .lines()
                .position(|line| line.contains(method))
                .map(|index| index + 1)
                .expect("fixture declares the method")
        })
        .collect();
    let actual: Vec<(String, usize)> = outcome
        .errors
        .iter()
        .map(|error| (error.code.clone(), error.line))
        .collect();
    let wanted: Vec<(String, usize)> = expected
        .into_iter()
        .map(|line| ("E0053".to_owned(), line))
        .collect();
    assert_eq!(actual, wanted, "{outcome}");
    assert!(
        outcome
            .errors
            .iter()
            .all(|error| error.file == "src/main.rs"),
        "{outcome}"
    );
}

/// A consumer crate outside this workspace with a path dependency on this
/// crate and its own target directory.
struct ConsumerProbe {
    root: PathBuf,
    target_dir: PathBuf,
    package: &'static str,
}

impl ConsumerProbe {
    fn new(package: &'static str) -> Self {
        let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join(package);
        let root = scratch.join("crate");
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("reset probe directory");
        }
        std::fs::create_dir_all(root.join("src")).expect("probe src");
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        std::fs::write(
            root.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{package}\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n\
                 [dependencies]\nsc-observability-types = {{ path = {:?} }}\n\n[workspace]\n",
                crate_dir.display().to_string()
            ),
        )
        .expect("probe manifest");
        // Start from the workspace lock so resolution reuses its pinned versions.
        let lock = crate_dir.join("../../Cargo.lock");
        if lock.is_file() {
            std::fs::copy(&lock, root.join("Cargo.lock")).expect("probe lockfile");
        }
        Self {
            root,
            target_dir: scratch.join("target"),
            package,
        }
    }

    fn check(&self, main_rs: &str) -> CheckOutcome {
        std::fs::write(self.root.join("src/main.rs"), main_rs).expect("probe main.rs");
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let output = Command::new(cargo)
            .args(["check", "--quiet", "--message-format=json"])
            .current_dir(&self.root)
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
            });
        }
        assert!(
            foreign.is_empty(),
            "a dependency failed to compile; infrastructure failure, not trait evidence\n{}\n{stderr}",
            foreign.join("\n")
        );
        assert!(
            output.status.success() || consumer_messages > 0,
            "cargo failed before compiling the probe (resolution, network or toolchain); \
             infrastructure failure, not trait evidence\n{stderr}"
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
            writeln!(f, "error[{}] {}:{}", error.code, error.file, error.line)?;
        }
        write!(f, "{}{}", self.rendered, self.stderr)
    }
}

struct ConsumerError {
    code: String,
    file: String,
    line: usize,
}
