#![expect(
    clippy::missing_errors_doc,
    reason = "builder error behavior is documented at the facade level, and repeating it on each constructor would add low-signal boilerplate"
)]
use std::sync::{Arc, Mutex, atomic::AtomicBool};

use sc_observability_types::typed::InitFailure;
use sc_observability_types::{ErrorContext, InitError, Remediation};

use crate::typed::{TypedLogSink, legacy_sink};
use crate::{
    ConsoleSink, JsonlFileSink, LevelControl, LevelOwner, Logger, LoggerConfig, LoggerRuntime,
    Running, SinkHealthState, SinkRegistration, default_log_path, error_codes,
};

impl SinkRegistration {
    /// Registers a typed sink through the retained open [`crate::LogSink`] boundary.
    ///
    /// The D13 adapter keeps the typed sink's structured diagnostic and source
    /// intact while this registration retains any sink-local filter metadata.
    #[must_use]
    pub fn typed(sink: Arc<dyn TypedLogSink>) -> Self {
        Self::new(legacy_sink(sink))
    }
}

/// Construction-time logger builder that owns sink registration.
#[expect(
    missing_debug_implementations,
    reason = "the builder stores registration trait objects whose debug representation is not part of the public API"
)]
pub struct LoggerBuilder {
    config: LoggerConfig,
    file_sink: Option<Arc<JsonlFileSink>>,
    sinks: Vec<SinkRegistration>,
    typed_sinks: Vec<Arc<dyn TypedLogSink>>,
}

/// Canonical initialization failure returned when sink registration is rejected.
pub type SinkRegistrationError = InitError;

impl LoggerBuilder {
    /// Creates a builder with the configured built-in sinks.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use sc_observability::{LoggerBuilder, LoggerConfig};
    /// use sc_observability_types::ServiceName;
    ///
    /// let builder = LoggerBuilder::new(LoggerConfig::default_for(
    ///     ServiceName::new("demo").expect("valid service"),
    ///     PathBuf::from("logs"),
    /// ))
    /// .expect("valid logger config");
    ///
    /// let _logger = builder.build();
    /// ```
    pub fn new(config: LoggerConfig) -> Result<Self, InitError> {
        let active_log_path = default_log_path(&config.log_root, &config.service_name);
        let mut sinks = Vec::new();
        let mut file_sink = None;

        if config.enable_file_sink {
            let sink = Arc::new(JsonlFileSink::for_logger(active_log_path));
            sinks.push(SinkRegistration::new(sink.clone()));
            file_sink = Some(sink);
        }

        if config.enable_console_sink {
            sinks.push(SinkRegistration::new(Arc::new(ConsoleSink::stdout())));
        }

        Ok(Self {
            config,
            file_sink,
            sinks,
            typed_sinks: Vec::new(),
        })
    }

    /// Registers one additional sink before the logger runtime is built.
    pub fn register_sink(&mut self, registration: SinkRegistration) -> &mut Self {
        self.sinks.push(registration);
        self
    }

    /// Registers a typed sink before the logger runtime is built.
    ///
    /// This is equivalent to registering [`SinkRegistration::typed`] and
    /// returns the builder so callers can continue fluent configuration.
    ///
    /// # Errors
    ///
    /// Returns a canonical initialization failure with a stable registration code
    /// when the sink is duplicated, degraded, or unavailable.
    pub fn register_typed_sink(
        &mut self,
        sink: Arc<dyn TypedLogSink>,
    ) -> Result<&mut Self, SinkRegistrationError> {
        if self
            .typed_sinks
            .iter()
            .any(|registered| Arc::ptr_eq(registered, &sink))
        {
            return Err(InitError::Configuration {
                context: Box::new(ErrorContext::new(
                    error_codes::SC_LOG_SINK_REGISTRATION_DUPLICATE,
                    "typed sink is already registered",
                    Remediation::recoverable(
                        "register each typed sink instance only once",
                        ["remove the duplicate registration"],
                    ),
                )),
            });
        }

        match sink.health().state {
            SinkHealthState::Healthy => {}
            SinkHealthState::DegradedDropping => {
                return Err(InitError::Configuration {
                    context: Box::new(ErrorContext::new(
                        error_codes::SC_LOG_SINK_REGISTRATION_INVALID,
                        "typed sink is degraded and cannot be registered",
                        Remediation::recoverable(
                            "restore the sink to a healthy state before registration",
                            ["repair the sink", "register a healthy sink"],
                        ),
                    )),
                });
            }
            SinkHealthState::Unavailable => {
                return Err(InitError::Configuration {
                    context: Box::new(ErrorContext::new(
                        error_codes::SC_LOG_SINK_REGISTRATION_CLOSED,
                        "typed sink is unavailable and closed to registration",
                        Remediation::recoverable(
                            "create a healthy replacement sink before registration",
                            ["create a replacement sink"],
                        ),
                    )),
                });
            }
        }

        self.typed_sinks.push(sink.clone());
        Ok(self.register_sink(SinkRegistration::typed(sink)))
    }

    /// Finalizes construction and returns the logger runtime.
    pub fn build(self) -> Result<Logger<Running>, InitError> {
        self.build_inner().map(|(logger, _)| logger)
    }

    /// Finalizes construction and returns the logger with weak level ownership.
    pub fn build_with_level_owner(
        self,
    ) -> Result<(Logger<Running>, LevelOwner), sc_observability_types::InitError> {
        let (logger, control) = self.build_inner()?;
        Ok((logger, LevelOwner::new(&control)))
    }

    fn build_inner(self) -> Result<(Logger<Running>, Arc<Mutex<LevelControl>>), InitError> {
        let Self {
            config,
            file_sink,
            sinks,
            typed_sinks: _,
        } = self;
        if sinks.is_empty() {
            return Err(InitError::Configuration {
                context: InitFailure::logger_initialization(
                    "logger must have at least one registered sink",
                    Remediation::recoverable(
                        "enable a built-in sink or register a sink before building the logger",
                        [
                            "set LoggerConfig.enable_file_sink or enable_console_sink to true",
                            "register a sink with LoggerBuilder::register_sink",
                        ],
                    ),
                )
                .into_context(),
            });
        }
        let config = Arc::new(config);
        let active_log_path = default_log_path(&config.log_root, &config.service_name);
        let query_available = active_log_path.exists() || config.enable_file_sink;
        let retained_log_policy = config.retained_log_policy;
        let runtime = LoggerRuntime::try_new(
            query_available,
            sinks.clone(),
            file_sink,
            retained_log_policy,
            config.queue_capacity.get(),
            #[cfg(test)]
            config.maintenance_test_pass_delay,
            #[cfg(test)]
            config.maintenance_test_pass_signal.clone(),
            #[cfg(test)]
            config.writer_start_should_fail,
        )
        .map_err(|failure| InitError::Runtime {
            context: failure.into_context(),
        })?;
        let diagnostic_admitter = runtime.diagnostic_admitter();
        let control = Arc::new(Mutex::new(LevelControl::new(&config, &diagnostic_admitter)));
        Ok((
            Logger {
                runtime,
                diagnostic_admitter: Some(diagnostic_admitter),
                config,
                sinks,
                shutdown: Arc::new(AtomicBool::new(false)),
                level_control: control.clone(),
                state: std::marker::PhantomData,
            },
            control,
        ))
    }
}
