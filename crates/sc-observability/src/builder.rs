#![expect(
    clippy::missing_errors_doc,
    reason = "builder error behavior is documented at the facade level, and repeating it on each constructor would add low-signal boilerplate"
)]
#![expect(
    clippy::must_use_candidate,
    reason = "builder methods are used immediately in fluent construction, so extra must_use decoration is intentionally omitted here"
)]

use std::sync::{Arc, Mutex, atomic::AtomicBool};

use sc_observability_types::typed::InitFailure;
#[allow(
    deprecated,
    reason = "the builder retains InitError in its published compatibility signature"
)]
use sc_observability_types::{InitError, Remediation};

use crate::typed::{TypedLogSink, legacy_sink};
use crate::{
    ConsoleSink, JsonlFileSink, LevelControl, LevelOwner, Logger, LoggerConfig, LoggerRuntime,
    Running, SinkRegistration, default_log_path,
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
}

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
    #[allow(
        deprecated,
        reason = "retained compatibility constructor keeps the published InitError signature"
    )]
    #[deprecated(
        since = "1.4.0",
        note = "Use LoggerBuilder::new_typed(); see migrate-error-api.md."
    )]
    pub fn new(config: LoggerConfig) -> Result<Self, InitError> {
        Self::new_typed(config).map_err(Into::into)
    }

    /// Creates a builder with the configured built-in sinks and typed failures.
    pub fn new_typed(config: LoggerConfig) -> Result<Self, InitFailure> {
        if config.queue_capacity == 0 {
            return Err(InitFailure::logger_initialization(
                "logger queue capacity must be greater than zero",
                Remediation::recoverable(
                    "set LoggerConfig.queue_capacity to a positive value before constructing the logger",
                    ["increase queue_capacity to at least 1"],
                ),
            ));
        }
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
    pub fn register_typed_sink(&mut self, sink: Arc<dyn TypedLogSink>) -> &mut Self {
        self.register_sink(SinkRegistration::typed(sink))
    }

    /// Finalizes construction and returns the logger runtime.
    ///
    /// # Panics
    ///
    /// Panics when the operating system cannot start the writer thread. New
    /// code that needs a recoverable startup error should use
    /// [`Self::build_typed`].
    pub fn build(self) -> Logger<Running> {
        self.build_typed()
            .expect("existing infallible builder expects writer thread startup")
    }

    /// Finalizes construction with a recoverable typed startup failure.
    pub fn build_typed(self) -> Result<Logger<Running>, InitFailure> {
        Ok(self.build_inner()?.0)
    }

    /// Finalizes construction and returns the logger with weak level ownership.
    #[allow(
        deprecated,
        reason = "supported owner-returning method keeps its published InitError signature"
    )]
    pub fn build_with_level_owner(
        self,
    ) -> Result<(Logger<Running>, LevelOwner), sc_observability_types::InitError> {
        self.build_with_level_owner_typed().map_err(Into::into)
    }

    /// Finalizes construction with weak level ownership and typed failures.
    pub fn build_with_level_owner_typed(
        self,
    ) -> Result<(Logger<Running>, LevelOwner), InitFailure> {
        let (logger, control) = self.build_inner()?;
        Ok((logger, LevelOwner::new(&control)))
    }

    fn build_inner(self) -> Result<(Logger<Running>, Arc<Mutex<LevelControl>>), InitFailure> {
        let Self {
            config,
            file_sink,
            sinks,
        } = self;
        let config = Arc::new(config);
        let active_log_path = default_log_path(&config.log_root, &config.service_name);
        let query_available = active_log_path.exists() || config.enable_file_sink;
        let retained_log_policy = config.retained_log_policy;
        let runtime = LoggerRuntime::try_new(
            query_available,
            sinks.clone(),
            file_sink,
            retained_log_policy,
            config.queue_capacity,
            #[cfg(test)]
            config.maintenance_test_pass_delay,
            #[cfg(test)]
            config.maintenance_test_pass_signal.clone(),
            #[cfg(test)]
            config.writer_start_should_fail,
        )?;
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
