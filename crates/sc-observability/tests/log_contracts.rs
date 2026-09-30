//! Wave-one contract fixtures for the D13 settings and open-sink surfaces.
//!
//! These are deliberately private test harnesses.  They freeze shape and
//! ownership without creating a second public error family before D12 installs
//! the canonical registry-backed 2.0 types.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sc_observability::{RetainedLogPolicy, SinkHealth};
use sc_observability_types::{
    ActionName, ErrorCode, ErrorContext, Level, LevelFilter, LogEvent,
    OBSERVATION_ENVELOPE_VERSION, ProcessIdentity, Remediation, SchemaVersion, ServiceName,
    SinkHealthState, SinkName, TargetCategory, Timestamp,
};
use serde::{Deserialize, Serialize};
use serde_json::Map;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct FixtureLogSettings {
    level: Option<LevelFilter>,
    log_root: Option<PathBuf>,
    enable_file_sink: Option<bool>,
    enable_console_sink: Option<bool>,
    retained_log_policy: Option<RetainedLogPolicy>,
}

#[derive(Debug, Clone, PartialEq)]
struct FixtureLogRoot(PathBuf);

impl FixtureLogRoot {
    fn new(path: PathBuf) -> Self {
        Self(path)
    }

    fn as_path(&self) -> &Path {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
struct FixtureResolvedLogSettings {
    level: LevelFilter,
    log_root: FixtureLogRoot,
    enable_file_sink: bool,
    enable_console_sink: bool,
    retained_log_policy: RetainedLogPolicy,
}

#[derive(Debug, Clone)]
struct FixtureLogSettingsInputs {
    file: Option<FixtureLogSettings>,
    shared_env: FixtureLogSettings,
    application_env: Option<FixtureLogSettings>,
    default_root: PathBuf,
}

impl FixtureLogSettings {
    // This private fixture preserves precedence shape only. Production
    // log_settings tests own validation of the effective resolved root.
    fn resolve(inputs: FixtureLogSettingsInputs) -> FixtureResolvedLogSettings {
        let FixtureLogSettingsInputs {
            file,
            shared_env,
            application_env,
            default_root,
        } = inputs;
        let application_env = application_env.unwrap_or_default();
        let file = file.unwrap_or_default();
        let level = application_env
            .level
            .or(shared_env.level)
            .or(file.level)
            .unwrap_or(LevelFilter::Info);
        // LOG-009's explicit JSON root exception is intentionally distinct from
        // the normal field precedence: a non-empty file root wins outright.
        let root = file
            .log_root
            .or(application_env.log_root)
            .or(shared_env.log_root)
            .unwrap_or(default_root);
        let enable_file_sink = application_env
            .enable_file_sink
            .or(shared_env.enable_file_sink)
            .or(file.enable_file_sink)
            .unwrap_or(true);
        let enable_console_sink = application_env
            .enable_console_sink
            .or(shared_env.enable_console_sink)
            .or(file.enable_console_sink)
            .unwrap_or(false);
        let retained_log_policy = application_env
            .retained_log_policy
            .or(shared_env.retained_log_policy)
            .or(file.retained_log_policy)
            .unwrap_or_default();

        FixtureResolvedLogSettings {
            level,
            log_root: FixtureLogRoot::new(root),
            enable_file_sink,
            enable_console_sink,
            retained_log_policy,
        }
    }
}

/// Private only: wave one checks object safety without defining a new public
/// typed-sink error or a duplicate classification surface.
trait FixtureSinkContract<E>: Send + Sync {
    fn write(&self, event: &LogEvent) -> Result<(), E>;
    fn flush(&self) -> Result<(), E>;
    fn health(&self) -> SinkHealth;
}

#[derive(Debug)]
struct FixtureHarnessError(Box<ErrorContext>);

impl FixtureHarnessError {
    fn context(&self) -> &ErrorContext {
        &self.0
    }
}

#[derive(Debug)]
enum FixtureSinkRegistrationError {
    Duplicate(FixtureHarnessError),
    Invalid(FixtureHarnessError),
    Closed(FixtureHarnessError),
}

impl FixtureSinkRegistrationError {
    fn payload(&self) -> &FixtureHarnessError {
        match self {
            Self::Duplicate(payload) | Self::Invalid(payload) | Self::Closed(payload) => payload,
        }
    }
}

fn assert_private_sink_object_safe(_: Arc<dyn FixtureSinkContract<FixtureHarnessError>>) {}

#[test]
fn settings_serde_defaults() {
    let settings: FixtureLogSettings =
        serde_json::from_str("{}").expect("empty settings deserialize");
    assert_eq!(settings, FixtureLogSettings::default());
    let encoded = serde_json::to_value(&settings).expect("settings serialize");
    assert_eq!(
        encoded,
        serde_json::json!({
            "level": null,
            "logRoot": null,
            "enableFileSink": null,
            "enableConsoleSink": null,
            "retainedLogPolicy": null,
        })
    );
    assert!(serde_json::from_str::<FixtureLogSettings>(r#"{"unknown":true}"#).is_err());
}

#[test]
fn log_root_validation() {
    let resolved = FixtureLogSettings::resolve(FixtureLogSettingsInputs {
        file: Some(FixtureLogSettings {
            log_root: Some(PathBuf::from("configured")),
            ..FixtureLogSettings::default()
        }),
        shared_env: FixtureLogSettings {
            log_root: Some(PathBuf::from("shared")),
            ..FixtureLogSettings::default()
        },
        application_env: Some(FixtureLogSettings {
            log_root: Some(PathBuf::from("application")),
            ..FixtureLogSettings::default()
        }),
        default_root: PathBuf::from("default"),
    });
    assert_eq!(resolved.log_root.as_path(), Path::new("configured"));
}

fn policy(
    rotation_max_bytes: u64,
    maintenance_max_work_per_pass: Option<usize>,
) -> RetainedLogPolicy {
    RetainedLogPolicy {
        rotation_max_bytes: sc_observability::ByteCount::from_bytes(rotation_max_bytes),
        maintenance_max_work_per_pass,
        ..RetainedLogPolicy::default()
    }
}

fn settings(
    level: Option<LevelFilter>,
    log_root: Option<&str>,
    enable_file_sink: Option<bool>,
    enable_console_sink: Option<bool>,
    retained_log_policy: Option<RetainedLogPolicy>,
) -> FixtureLogSettings {
    FixtureLogSettings {
        level,
        log_root: log_root.map(PathBuf::from),
        enable_file_sink,
        enable_console_sink,
        retained_log_policy,
    }
}

struct FixtureResolutionCase {
    name: &'static str,
    file: Option<FixtureLogSettings>,
    shared_env: FixtureLogSettings,
    application_env: Option<FixtureLogSettings>,
    expected_level: LevelFilter,
    expected_root: &'static str,
    expected_file_sink: bool,
    expected_console_sink: bool,
    expected_policy: RetainedLogPolicy,
}

fn settings_resolution_cases() -> [FixtureResolutionCase; 5] {
    let default_policy = RetainedLogPolicy::default();
    let json_policy = policy(1_024, Some(1));
    let shared_policy = policy(2_048, Some(2));
    let application_policy = policy(4_096, Some(3));
    [
        FixtureResolutionCase {
            name: "defaults apply when every source is absent",
            file: None,
            shared_env: FixtureLogSettings::default(),
            application_env: None,
            expected_level: LevelFilter::Info,
            expected_root: "default",
            expected_file_sink: true,
            expected_console_sink: false,
            expected_policy: default_policy,
        },
        FixtureResolutionCase {
            name: "JSON supplies values when environments are absent",
            file: Some(settings(
                Some(LevelFilter::Debug),
                Some("json"),
                Some(false),
                Some(true),
                Some(json_policy),
            )),
            shared_env: FixtureLogSettings::default(),
            application_env: None,
            expected_level: LevelFilter::Debug,
            expected_root: "json",
            expected_file_sink: false,
            expected_console_sink: true,
            expected_policy: json_policy,
        },
        FixtureResolutionCase {
            name: "SC environment overrides JSON for ordinary fields",
            file: Some(settings(
                Some(LevelFilter::Debug),
                None,
                Some(false),
                Some(true),
                Some(json_policy),
            )),
            shared_env: settings(
                Some(LevelFilter::Warn),
                Some("shared"),
                Some(true),
                Some(false),
                Some(shared_policy),
            ),
            application_env: None,
            expected_level: LevelFilter::Warn,
            expected_root: "shared",
            expected_file_sink: true,
            expected_console_sink: false,
            expected_policy: shared_policy,
        },
        FixtureResolutionCase {
            name: "application environment overrides every ordinary field",
            file: Some(settings(
                Some(LevelFilter::Debug),
                None,
                Some(false),
                Some(true),
                Some(json_policy),
            )),
            shared_env: settings(
                Some(LevelFilter::Warn),
                Some("shared"),
                Some(true),
                Some(false),
                Some(shared_policy),
            ),
            application_env: Some(settings(
                Some(LevelFilter::Error),
                Some("application"),
                Some(false),
                Some(true),
                Some(application_policy),
            )),
            expected_level: LevelFilter::Error,
            expected_root: "application",
            expected_file_sink: false,
            expected_console_sink: true,
            expected_policy: application_policy,
        },
        FixtureResolutionCase {
            name: "a non-empty JSON root wins over both environments",
            file: Some(settings(None, Some("json"), None, None, None)),
            shared_env: settings(None, Some("shared"), None, None, None),
            application_env: Some(settings(None, Some("application"), None, None, None)),
            expected_level: LevelFilter::Info,
            expected_root: "json",
            expected_file_sink: true,
            expected_console_sink: false,
            expected_policy: default_policy,
        },
    ]
}

#[test]
fn settings_resolution_contract_table() {
    for case in settings_resolution_cases() {
        let resolved = FixtureLogSettings::resolve(FixtureLogSettingsInputs {
            file: case.file,
            shared_env: case.shared_env,
            application_env: case.application_env,
            default_root: PathBuf::from("default"),
        });
        assert_eq!(resolved.level, case.expected_level, "{}", case.name);
        assert_eq!(
            resolved.log_root.as_path(),
            Path::new(case.expected_root),
            "{}",
            case.name
        );
        assert_eq!(
            resolved.enable_file_sink, case.expected_file_sink,
            "{}",
            case.name
        );
        assert_eq!(
            resolved.enable_console_sink, case.expected_console_sink,
            "{}",
            case.name
        );
        assert_eq!(
            resolved.retained_log_policy, case.expected_policy,
            "{}",
            case.name
        );
    }
}

#[test]
fn settings_null_empty_environment_and_atomic_policy_contracts() {
    let null_json: FixtureLogSettings = serde_json::from_str(
        r#"{
            "level": null,
            "logRoot": null,
            "enableFileSink": null,
            "enableConsoleSink": null,
            "retainedLogPolicy": null
        }"#,
    )
    .expect("null JSON fields deserialize as absent overrides");
    assert_eq!(null_json, FixtureLogSettings::default());

    let shared_policy = policy(2_048, Some(2));
    let application_policy = policy(4_096, Some(3));
    let resolved = FixtureLogSettings::resolve(FixtureLogSettingsInputs {
        file: Some(settings(
            None,
            None,
            None,
            None,
            Some(policy(1_024, Some(1))),
        )),
        shared_env: settings(
            Some(LevelFilter::Warn),
            Some("shared"),
            Some(false),
            Some(true),
            Some(shared_policy),
        ),
        application_env: Some(null_json),
        default_root: PathBuf::from("default"),
    });
    assert_eq!(resolved.level, LevelFilter::Warn);
    assert_eq!(resolved.log_root.as_path(), Path::new("shared"));
    assert!(!resolved.enable_file_sink);
    assert!(resolved.enable_console_sink);
    assert_eq!(resolved.retained_log_policy, shared_policy);

    let resolved = FixtureLogSettings::resolve(FixtureLogSettingsInputs {
        file: Some(settings(
            None,
            None,
            None,
            None,
            Some(policy(1_024, Some(1))),
        )),
        shared_env: settings(None, None, None, None, Some(shared_policy)),
        application_env: Some(settings(None, None, None, None, Some(application_policy))),
        default_root: PathBuf::from("default"),
    });
    assert_eq!(
        resolved.retained_log_policy, application_policy,
        "the winning retainedLogPolicy replaces the complete policy instead of merging fields"
    );
}

fn contract_event() -> LogEvent {
    LogEvent {
        version: SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION).expect("static schema version"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service: ServiceName::new("contract").expect("static service name"),
        target: TargetCategory::new("contract").expect("static target category"),
        action: ActionName::new("write").expect("static action name"),
        message: None,
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: None,
        fields: Map::default(),
    }
}

#[test]
fn private_sink_contract_object_safety() {
    struct FixtureContractSink;

    impl FixtureSinkContract<FixtureHarnessError> for FixtureContractSink {
        fn write(&self, _: &LogEvent) -> Result<(), FixtureHarnessError> {
            Ok(())
        }

        fn flush(&self) -> Result<(), FixtureHarnessError> {
            Ok(())
        }

        fn health(&self) -> SinkHealth {
            SinkHealth {
                name: SinkName::new("contract").expect("static sink name"),
                state: SinkHealthState::Healthy,
                last_error: None,
            }
        }
    }

    let sink: Arc<dyn FixtureSinkContract<FixtureHarnessError>> = Arc::new(FixtureContractSink);
    assert_private_sink_object_safe(Arc::clone(&sink));
    sink.write(&contract_event()).expect("write contract");
    sink.flush().expect("flush contract");
    assert_eq!(sink.health().state, SinkHealthState::Healthy);
}

#[test]
fn registration_error_payloads() {
    let context = Box::new(
        ErrorContext::new(
            ErrorCode::new_static("SC_LOG_SINK_REGISTRATION_DUPLICATE"),
            "duplicate sink",
            Remediation::recoverable("use one registration", ["remove the duplicate"]),
        )
        .source(Box::new(std::io::Error::other("already registered"))),
    );
    let error = FixtureSinkRegistrationError::Duplicate(FixtureHarnessError(context));
    let payload = error.payload();
    assert_eq!(
        payload.context().diagnostic().code.as_str(),
        "SC_LOG_SINK_REGISTRATION_DUPLICATE"
    );
    assert_eq!(
        std::error::Error::source(payload.context())
            .map(ToString::to_string)
            .as_deref(),
        Some("already registered")
    );

    for (error, expected_code) in [
        (
            FixtureSinkRegistrationError::Invalid(FixtureHarnessError(Box::new(
                ErrorContext::new(
                    ErrorCode::new_static("SC_LOG_SINK_REGISTRATION_INVALID"),
                    "invalid sink",
                    Remediation::not_recoverable("repair the sink contract"),
                ),
            ))),
            "SC_LOG_SINK_REGISTRATION_INVALID",
        ),
        (
            FixtureSinkRegistrationError::Closed(FixtureHarnessError(Box::new(ErrorContext::new(
                ErrorCode::new_static("SC_LOG_SINK_REGISTRATION_CLOSED"),
                "closed logger",
                Remediation::not_recoverable("register before shutdown"),
            )))),
            "SC_LOG_SINK_REGISTRATION_CLOSED",
        ),
    ] {
        assert_eq!(
            error.payload().context().diagnostic().code.as_str(),
            expected_code
        );
    }
}

#[test]
fn contract_harness_preserves_context() {
    struct FixtureRejectingSink;

    impl FixtureSinkContract<FixtureHarnessError> for FixtureRejectingSink {
        fn write(&self, _: &LogEvent) -> Result<(), FixtureHarnessError> {
            Err(FixtureHarnessError(Box::new(
                ErrorContext::new(
                    ErrorCode::new_static("D13_FIXTURE_LOG_SINK_CONTRACT"),
                    "contract sink rejected the event",
                    Remediation::recoverable("repair the sink", ["retry registration"]),
                )
                .source(Box::new(std::io::Error::other("fixture source"))),
            )))
        }

        fn flush(&self) -> Result<(), FixtureHarnessError> {
            Ok(())
        }

        fn health(&self) -> SinkHealth {
            SinkHealth {
                name: SinkName::new("rejecting-contract").expect("static sink name"),
                state: SinkHealthState::Healthy,
                last_error: None,
            }
        }
    }

    let sink: Arc<dyn FixtureSinkContract<FixtureHarnessError>> = Arc::new(FixtureRejectingSink);
    let payload = sink
        .write(&contract_event())
        .expect_err("rejecting fixture sink returns its context");
    assert_eq!(
        payload.context().diagnostic().code.as_str(),
        "D13_FIXTURE_LOG_SINK_CONTRACT"
    );
    assert_eq!(
        payload.context().diagnostic().remediation,
        Remediation::recoverable("repair the sink", ["retry registration"])
    );
    assert_eq!(
        std::error::Error::source(payload.context())
            .map(ToString::to_string)
            .as_deref(),
        Some("fixture source")
    );
}
