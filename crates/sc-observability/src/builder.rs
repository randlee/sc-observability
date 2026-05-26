#![expect(
    clippy::missing_errors_doc,
    reason = "builder error behavior is documented at the facade level, and repeating it on each constructor would add low-signal boilerplate"
)]
#![expect(
    clippy::must_use_candidate,
    reason = "builder methods are used immediately in fluent construction, so extra must_use decoration is intentionally omitted here"
)]

use std::sync::{Arc, atomic::AtomicBool};

use sc_observability_types::InitError;

use crate::{
    ConsoleSink, JsonlFileSink, Logger, LoggerConfig, LoggerRuntime, Running, SinkRegistration,
    default_log_path,
};

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
        })
    }

    /// Registers one additional sink before the logger runtime is built.
    pub fn register_sink(&mut self, registration: SinkRegistration) -> &mut Self {
        self.sinks.push(registration);
        self
    }

    /// Finalizes construction and returns the logger runtime.
    pub fn build(self) -> Logger<Running> {
        let Self {
            config,
            file_sink,
            sinks,
        } = self;
        let active_log_path = default_log_path(&config.log_root, &config.service_name);
        let query_available = active_log_path.exists() || config.enable_file_sink;
        let retained_log_policy = config.retained_log_policy;
        Logger {
            runtime: LoggerRuntime::new(
                query_available,
                sinks.clone(),
                file_sink,
                retained_log_policy,
                config.queue_capacity,
                #[cfg(test)]
                config.maintenance_test_pass_delay,
            ),
            config,
            sinks,
            shutdown: Arc::new(AtomicBool::new(false)),
            state: std::marker::PhantomData,
        }
    }
}
