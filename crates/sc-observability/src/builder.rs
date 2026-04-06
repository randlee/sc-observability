use std::sync::{Arc, atomic::AtomicBool};

use sc_observability_types::InitError;

use crate::{
    ConsoleSink, JsonlFileSink, Logger, LoggerConfig, LoggerRuntime, SinkRegistration,
    default_log_path,
};

/// Construction-time logger builder that owns sink registration.
pub struct LoggerBuilder {
    config: LoggerConfig,
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

        if config.enable_file_sink {
            let sink = JsonlFileSink::new(active_log_path, config.rotation, config.retention);
            sinks.push(SinkRegistration::new(Arc::new(sink)));
        }

        if config.enable_console_sink {
            sinks.push(SinkRegistration::new(Arc::new(ConsoleSink::stdout())));
        }

        Ok(Self { config, sinks })
    }

    /// Registers one additional sink before the logger runtime is built.
    pub fn register_sink(&mut self, registration: SinkRegistration) -> &mut Self {
        self.sinks.push(registration);
        self
    }

    /// Finalizes construction and returns the logger runtime.
    pub fn build(self) -> Logger {
        let active_log_path = default_log_path(&self.config.log_root, &self.config.service_name);
        let query_available = active_log_path.exists() || self.config.enable_file_sink;
        Logger {
            config: self.config,
            sinks: self.sinks,
            shutdown: Arc::new(AtomicBool::new(false)),
            runtime: LoggerRuntime::new(query_available),
        }
    }
}
