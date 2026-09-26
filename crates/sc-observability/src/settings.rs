//! Serde-stable startup logging settings declarations.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use sc_observability_types::{EnvPrefix, ErrorCode, ErrorContext, LevelFilter, Remediation};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::RetainedLogPolicy;

/// Optional logging overrides accepted from JSON or an environment namespace.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct LogSettings {
    /// Minimum level admitted by the logger.
    pub level: Option<LevelFilter>,
    /// Root directory for service logs.
    pub log_root: Option<PathBuf>,
    /// Enables the built-in JSONL file sink.
    pub enable_file_sink: Option<bool>,
    /// Enables the built-in console sink.
    pub enable_console_sink: Option<bool>,
    /// Atomic retained-log policy override.
    pub retained_log_policy: Option<RetainedLogPolicy>,
}

/// A non-empty log-root path validated during settings resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRoot(PathBuf);

impl LogRoot {
    pub(crate) fn new(path: PathBuf) -> Result<Self, LogSettingsError> {
        if path.as_os_str().is_empty() {
            return Err(LogSettingsError::invalid_value("logRoot must not be empty"));
        }
        Ok(Self(path))
    }

    pub(crate) fn into_path(self) -> PathBuf {
        self.0
    }
}

impl AsRef<Path> for LogRoot {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

/// Fully resolved startup settings with no optional fields.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedLogSettings {
    /// Resolved logger level.
    pub level: LevelFilter,
    /// Validated root directory.
    pub log_root: LogRoot,
    /// Resolved built-in file-sink flag.
    pub enable_file_sink: bool,
    /// Resolved built-in console-sink flag.
    pub enable_console_sink: bool,
    /// Resolved retained-log policy.
    pub retained_log_policy: RetainedLogPolicy,
}

/// Immutable process-environment input captured once for one resolution.
#[derive(Debug, Clone, Default)]
pub struct EnvSnapshot(pub(crate) Vec<(OsString, OsString)>);

impl EnvSnapshot {
    /// Captures the process environment once.
    #[must_use]
    pub fn capture() -> Self {
        Self(std::env::vars_os().collect())
    }

    /// Builds one deterministic snapshot from supplied key/value pairs.
    #[must_use]
    pub fn from_pairs(values: impl IntoIterator<Item = (OsString, OsString)>) -> Self {
        Self(values.into_iter().collect())
    }
}

/// Inputs to deterministic settings resolution.
#[derive(Debug, Clone)]
pub struct LogSettingsInputs {
    /// Optional JSON/file settings.
    pub file: Option<LogSettings>,
    /// Settings parsed from the shared `SC` namespace.
    pub shared_env: LogSettings,
    /// Optional settings parsed from an application namespace.
    pub application_env: Option<LogSettings>,
    /// Root used when no source overrides it.
    pub default_root: PathBuf,
}

/// Stable startup-settings failure categories with preserved diagnostics.
#[non_exhaustive]
#[derive(Debug, PartialEq, Error)]
pub enum LogSettingsError {
    /// The application namespace collides with the shared `SC` namespace.
    #[error("{context}")]
    PrefixCollision {
        /// Preserved collision diagnostic and recovery guidance.
        #[source]
        context: Box<ErrorContext>,
    },
    /// The selected environment contains invalid UTF-8, duplicates, or syntax.
    #[error("{context}")]
    InvalidEnvironment {
        /// Preserved environment diagnostic and recovery guidance.
        #[source]
        context: Box<ErrorContext>,
    },
    /// The selected environment namespace contains an unsupported setting key.
    #[error("{context}")]
    UnknownKey {
        /// Preserved unsupported-key diagnostic and recovery guidance.
        #[source]
        context: Box<ErrorContext>,
    },
    /// A supplied JSON or environment value is invalid.
    #[error("{context}")]
    InvalidValue {
        /// Preserved invalid-value diagnostic and recovery guidance.
        #[source]
        context: Box<ErrorContext>,
    },
    /// Inputs cannot be combined into one resolved settings value.
    #[error("{context}")]
    Resolution {
        /// Preserved resolution diagnostic and recovery guidance.
        #[source]
        context: Box<ErrorContext>,
    },
}

impl LogSettingsError {
    /// Returns the preserved diagnostic context.
    #[must_use]
    pub fn context(&self) -> &ErrorContext {
        match self {
            Self::PrefixCollision { context }
            | Self::InvalidEnvironment { context }
            | Self::UnknownKey { context }
            | Self::InvalidValue { context }
            | Self::Resolution { context } => context,
        }
    }

    /// Returns this failure's stable machine-readable code.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        self.context().diagnostic().code.clone()
    }

    pub(crate) fn invalid_value(message: impl Into<String>) -> Self {
        Self::InvalidValue {
            context: Box::new(ErrorContext::new(
                ErrorCode::new_static("SC_LOG_SETTINGS_INVALID_VALUE"),
                message,
                Remediation::recoverable(
                    "correct the supplied logging setting",
                    std::iter::empty::<String>(),
                ),
            )),
        }
    }

    pub(crate) fn environment(message: impl Into<String>) -> Self {
        Self::InvalidEnvironment {
            context: Box::new(ErrorContext::new(
                ErrorCode::new_static("SC_LOG_SETTINGS_INVALID_ENVIRONMENT"),
                message,
                Remediation::recoverable(
                    "correct the selected logging environment namespace",
                    std::iter::empty::<String>(),
                ),
            )),
        }
    }

    pub(crate) fn unknown_key(message: impl Into<String>) -> Self {
        Self::UnknownKey {
            context: Box::new(ErrorContext::new(
                ErrorCode::new_static("SC_LOG_SETTINGS_UNKNOWN_KEY"),
                message,
                Remediation::recoverable(
                    "remove or rename the unsupported logging environment key",
                    std::iter::empty::<String>(),
                ),
            )),
        }
    }

    pub(crate) fn prefix_collision(prefix: &EnvPrefix) -> Self {
        Self::PrefixCollision {
            context: Box::new(ErrorContext::new(
                ErrorCode::new_static("SC_LOG_SETTINGS_PREFIX_COLLISION"),
                format!("application prefix {} collides with SC", prefix.as_str()),
                Remediation::recoverable(
                    "choose an application prefix other than SC",
                    std::iter::empty::<String>(),
                ),
            )),
        }
    }
}
