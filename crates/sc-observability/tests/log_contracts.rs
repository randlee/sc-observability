//! Wave-one contract fixtures for the D13 settings and open-sink surfaces.
//!
//! These are deliberately private test harnesses.  They freeze shape and
//! ownership without creating a second public error family before D12 installs
//! the canonical registry-backed 2.0 types.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sc_observability::{RetainedLogPolicy, SinkHealth};
use sc_observability_types::{
    ErrorCode, ErrorContext, LevelFilter, Remediation, SinkHealthState, SinkName,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct LogSettings {
    level: Option<LevelFilter>,
    log_root: Option<PathBuf>,
    enable_file_sink: Option<bool>,
    enable_console_sink: Option<bool>,
    retained_log_policy: Option<RetainedLogPolicy>,
}

#[derive(Debug, Clone, PartialEq)]
struct LogRoot(PathBuf);

impl LogRoot {
    fn new(path: PathBuf) -> Result<Self, LogSettingsError> {
        if path.as_os_str().is_empty() {
            return Err(LogSettingsError::InvalidValue);
        }
        Ok(Self(path))
    }

    fn as_path(&self) -> &Path {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ResolvedLogSettings {
    level: LevelFilter,
    log_root: LogRoot,
    enable_file_sink: bool,
    enable_console_sink: bool,
    retained_log_policy: RetainedLogPolicy,
}

#[derive(Debug, Clone, Default)]
struct EnvSnapshot(BTreeMap<String, String>);

#[derive(Debug, Clone)]
struct LogSettingsInputs {
    file: Option<LogSettings>,
    shared_env: LogSettings,
    application_env: Option<LogSettings>,
    default_root: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogSettingsError {
    PrefixCollision,
    InvalidEnvironment,
    UnknownKey,
    InvalidValue,
    Resolution,
}

impl LogSettingsError {
    const fn code(self) -> &'static str {
        match self {
            Self::PrefixCollision => "LOG-001",
            Self::InvalidEnvironment => "LOG-002",
            Self::UnknownKey => "LOG-003",
            Self::InvalidValue => "LOG-004",
            Self::Resolution => "LOG-005",
        }
    }
}

impl LogSettings {
    fn resolve(inputs: LogSettingsInputs) -> Result<ResolvedLogSettings, LogSettingsError> {
        let LogSettingsInputs {
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
            .filter(|root| !root.as_os_str().is_empty())
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

        Ok(ResolvedLogSettings {
            level,
            log_root: LogRoot::new(root)?,
            enable_file_sink,
            enable_console_sink,
            retained_log_policy,
        })
    }
}

/// Private only: wave one checks object safety without defining a new public
/// typed-sink error or a duplicate classification surface.
trait PrivateSinkContract<E>: Send + Sync {
    fn write(&self) -> Result<(), E>;
    fn flush(&self) -> Result<(), E>;
    fn health(&self) -> SinkHealth;
}

#[derive(Debug)]
struct HarnessError(Box<ErrorContext>);

impl HarnessError {
    fn context(&self) -> &ErrorContext {
        &self.0
    }
}

#[derive(Debug)]
enum SinkRegistrationError {
    Duplicate(HarnessError),
    Invalid(HarnessError),
    Closed(HarnessError),
}

impl SinkRegistrationError {
    fn payload(&self) -> &HarnessError {
        match self {
            Self::Duplicate(payload) | Self::Invalid(payload) | Self::Closed(payload) => payload,
        }
    }
}

fn assert_private_sink_object_safe(_: Arc<dyn PrivateSinkContract<HarnessError>>) {}

#[test]
fn settings_serde_defaults() {
    let settings: LogSettings = serde_json::from_str("{}").expect("empty settings deserialize");
    assert_eq!(settings, LogSettings::default());
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
    assert!(serde_json::from_str::<LogSettings>(r#"{"unknown":true}"#).is_err());
}

#[test]
fn log_root_validation() {
    let resolved = LogSettings::resolve(LogSettingsInputs {
        file: Some(LogSettings {
            log_root: Some(PathBuf::from("configured")),
            ..LogSettings::default()
        }),
        shared_env: LogSettings {
            log_root: Some(PathBuf::from("shared")),
            ..LogSettings::default()
        },
        application_env: Some(LogSettings {
            log_root: Some(PathBuf::from("application")),
            ..LogSettings::default()
        }),
        default_root: PathBuf::from("default"),
    })
    .expect("configured root resolves");
    assert_eq!(resolved.log_root.as_path(), Path::new("configured"));

    let error = LogRoot::new(PathBuf::new()).expect_err("empty root is rejected");
    assert_eq!(error, LogSettingsError::InvalidValue);
    assert_eq!(error.code(), "LOG-004");
}

#[test]
fn private_sink_contract_object_safety() {
    struct ContractSink;

    impl PrivateSinkContract<HarnessError> for ContractSink {
        fn write(&self) -> Result<(), HarnessError> {
            Ok(())
        }

        fn flush(&self) -> Result<(), HarnessError> {
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

    let sink: Arc<dyn PrivateSinkContract<HarnessError>> = Arc::new(ContractSink);
    assert_private_sink_object_safe(Arc::clone(&sink));
    sink.write().expect("write contract");
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
    let error = SinkRegistrationError::Duplicate(HarnessError(context));
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

    for error in [
        SinkRegistrationError::Invalid(HarnessError(Box::new(ErrorContext::new(
            ErrorCode::new_static("SC_LOG_SINK_REGISTRATION_INVALID"),
            "invalid sink",
            Remediation::not_recoverable("repair the sink contract"),
        )))),
        SinkRegistrationError::Closed(HarnessError(Box::new(ErrorContext::new(
            ErrorCode::new_static("SC_LOG_SINK_REGISTRATION_CLOSED"),
            "closed logger",
            Remediation::not_recoverable("register before shutdown"),
        )))),
    ] {
        assert!(
            error
                .payload()
                .context()
                .diagnostic()
                .code
                .as_str()
                .starts_with("SC_LOG_")
        );
    }
}

#[test]
fn contract_harness_preserves_context() {
    let snapshot = EnvSnapshot::default();
    assert!(snapshot.0.is_empty());
    for error in [
        LogSettingsError::PrefixCollision,
        LogSettingsError::InvalidEnvironment,
        LogSettingsError::UnknownKey,
        LogSettingsError::InvalidValue,
        LogSettingsError::Resolution,
    ] {
        assert!(error.code().starts_with("LOG-"));
    }
}
