//! Native OpenTelemetry integration for `sc-observability` (ADR-023).
//!
//! With `log-sink`, `OtelLogSink` maps already-redacted core log events to
//! native OpenTelemetry log records from a caller-owned SDK logger provider.
//! With `synchronous-client`, `sync::Client` exports logs, spans and metrics
//! through the official blocking OTLP/HTTP protobuf exporter for frontends
//! without a Tokio runtime. Native Tokio hosts use the unmodified upstream
//! re-exports: `api` and `sdk` with `native`, and `otlp` with
//! `tokio-exporter`.

pub mod constants;
pub mod error_codes;

#[cfg(feature = "log-sink")]
mod log_sink;
#[cfg(feature = "native")]
mod native;
#[cfg(feature = "log-sink")]
mod severity;
#[cfg(feature = "synchronous-client")]
pub mod sync;

#[cfg(feature = "log-sink")]
#[doc(inline)]
pub use log_sink::OtelLogSink;
#[cfg(feature = "tokio-exporter")]
#[doc(inline)]
pub use native::otlp;
#[cfg(feature = "native")]
#[doc(inline)]
pub use native::{api, sdk};
