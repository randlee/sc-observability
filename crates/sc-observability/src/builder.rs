#![expect(
    clippy::missing_errors_doc,
    reason = "builder error behavior is documented at the facade level, and repeating it on each constructor would add low-signal boilerplate"
)]
use std::sync::{Arc, Mutex, atomic::AtomicBool};

use sc_observability_types::typed::InitFailure;
use sc_observability_types::{ErrorContext, Remediation, v2::InitError as CanonicalInitError};

use crate::sink::LogSink;
use crate::{
    CanonicalLogger, ConsoleSink, JsonlFileSink, LevelControl, LevelOwner, LoggerConfig,
    LoggerRuntime, QueueCapacity, Running, SinkHealthState, SinkRegistration, default_log_path,
    error_codes,
};

impl SinkRegistration {
    /// Registers a canonical sink exactly as provided.
    ///
    /// The registration stores the input [`Arc`] directly, with no adapter, so
    /// the canonical sink's structured diagnostic and source reach the runtime
    /// unchanged. Released [`crate::LogSink`] values register through
    /// [`SinkRegistration::new`] instead.
    #[must_use]
    pub fn typed(sink: Arc<dyn LogSink>) -> Self {
        Self { sink, filter: None }
    }
}

/// Construction-time logger builder that owns sink registration.
#[expect(
    missing_debug_implementations,
    reason = "the builder stores registration trait objects whose debug representation is not part of the public API"
)]
pub struct CanonicalLoggerBuilder {
    config: LoggerConfig,
    file_sink: Option<Arc<JsonlFileSink>>,
    sinks: Vec<SinkRegistration>,
}

/// Canonical initialization failure returned when sink registration is rejected.
pub type SinkRegistrationError = CanonicalInitError;

impl CanonicalLoggerBuilder {
    /// Creates a builder with the configured built-in sinks.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use sc_observability::{LoggerConfig, v2::LoggerBuilder};
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
    pub fn new(config: LoggerConfig) -> Result<Self, CanonicalInitError> {
        let active_log_path = default_log_path(&config.log_root, &config.service_name);
        let mut sinks = Vec::new();
        let mut file_sink = None;

        if config.enable_file_sink {
            let sink = Arc::new(JsonlFileSink::for_logger(active_log_path));
            sinks.push(SinkRegistration::typed(sink.clone()));
            file_sink = Some(sink);
        }

        if config.enable_console_sink {
            sinks.push(SinkRegistration::typed(Arc::new(ConsoleSink::stdout())));
        }

        Ok(Self {
            config,
            file_sink,
            sinks,
        })
    }

    /// Creates a builder with the released typed initialization failure.
    pub fn new_typed(config: LoggerConfig) -> Result<Self, InitFailure> {
        Self::new(config).map_err(|error| InitFailure::from_context(error.into_context()))
    }

    /// Registers one additional sink before the logger runtime is built.
    ///
    /// This is the released infallible registration path: the registration is
    /// stored as provided, including its filter, without the duplicate or
    /// health validation that [`Self::register_typed_sink`] performs.
    pub fn register_sink(&mut self, registration: SinkRegistration) -> &mut Self {
        self.sinks.push(registration);
        self
    }

    /// Registers a canonical sink before the logger runtime is built.
    ///
    /// The canonical owner of this registration is [`crate::v2::LoggerBuilder`]
    /// and the sink trait is [`crate::v2::LogSink`]. This is equivalent to
    /// registering [`SinkRegistration::typed`] after validating the exact
    /// stored [`Arc`] against every sink already registered, including
    /// built-ins and raw [`Self::register_sink`] registrations, and returns the
    /// builder so callers can continue fluent configuration.
    ///
    /// # Errors
    ///
    /// Returns a canonical initialization failure with a stable registration code
    /// when the sink is duplicated, degraded, or unavailable.
    pub fn register_typed_sink(
        &mut self,
        sink: Arc<dyn LogSink>,
    ) -> Result<&mut Self, SinkRegistrationError> {
        if self
            .sinks
            .iter()
            .any(|registered| Arc::ptr_eq(&registered.sink, &sink))
        {
            return Err(CanonicalInitError::Configuration {
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
                return Err(CanonicalInitError::Configuration {
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
                return Err(CanonicalInitError::Configuration {
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

        Ok(self.register_sink(SinkRegistration::typed(sink)))
    }

    /// Finalizes construction with the canonical recoverable error surface.
    pub fn build(self) -> Result<CanonicalLogger<Running>, CanonicalInitError> {
        self.build_inner().map(|(logger, _)| logger)
    }

    /// Finalizes construction with the released typed initialization failure.
    pub fn build_typed(self) -> Result<CanonicalLogger<Running>, InitFailure> {
        self.build()
            .map_err(|error| InitFailure::from_context(error.into_context()))
    }

    /// Finalizes construction and returns the logger with weak level ownership.
    pub fn build_with_level_owner(
        self,
    ) -> Result<(CanonicalLogger<Running>, LevelOwner), CanonicalInitError> {
        let (logger, control) = self.build_inner()?;
        Ok((logger, LevelOwner::new(&control)))
    }

    /// Finalizes construction with the released typed initialization failure.
    pub fn build_with_level_owner_typed(
        self,
    ) -> Result<(CanonicalLogger<Running>, LevelOwner), InitFailure> {
        self.build_with_level_owner()
            .map_err(|error| InitFailure::from_context(error.into_context()))
    }

    fn build_inner(
        self,
    ) -> Result<(CanonicalLogger<Running>, Arc<Mutex<LevelControl>>), CanonicalInitError> {
        let Self {
            config,
            file_sink,
            sinks,
        } = self;
        if sinks.is_empty() {
            return Err(CanonicalInitError::Configuration {
                context: InitFailure::logger_initialization(
                    "logger must have at least one registered sink",
                    Remediation::recoverable(
                        "enable a built-in sink or register a sink before building the logger",
                        [
                            "set LoggerConfig.enable_file_sink or enable_console_sink to true",
                            "register a sink with v2::LoggerBuilder::register_sink",
                        ],
                    ),
                )
                .into_context(),
            });
        }
        let queue_capacity = QueueCapacity::new(config.queue_capacity).ok_or_else(|| {
            CanonicalInitError::Configuration {
                context: InitFailure::logger_initialization(
                    "logger queue capacity must be positive",
                    Remediation::recoverable(
                        "set LoggerConfig.queue_capacity to a positive value",
                        ["use a queue capacity of at least one record"],
                    ),
                )
                .into_context(),
            }
        })?;
        let config = Arc::new(config);
        let active_log_path = default_log_path(&config.log_root, &config.service_name);
        let query_available = active_log_path.exists() || config.enable_file_sink;
        let retained_log_policy = config.retained_log_policy;
        let runtime = LoggerRuntime::try_new(
            query_available,
            sinks.clone(),
            file_sink,
            retained_log_policy,
            queue_capacity.get(),
            #[cfg(test)]
            config.maintenance_test_pass_delay,
            #[cfg(test)]
            config.maintenance_test_pass_signal.clone(),
            #[cfg(test)]
            config.writer_start_should_fail,
        )
        .map_err(|failure| CanonicalInitError::Runtime {
            context: failure.into_context(),
        })?;
        let diagnostic_admitter = runtime.diagnostic_admitter();
        let control = Arc::new(Mutex::new(LevelControl::new(&config, &diagnostic_admitter)));
        Ok((
            CanonicalLogger {
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
