//! Shared telemetry input, admission, delivery and configuration contracts.
mod canonical;
mod client;
mod config;
mod envelope;
pub mod error_codes;
mod errors;
#[doc(inline)]
pub use canonical::{IdSource, SystemIds};
#[doc(inline)]
pub use client::{
    AdmissionReceipt, DeliveryState, DeliveryStatus, EnvelopeVersion, FlushReport, LeaseInfo,
    RecordKey, Representation, Signal, SignalCounts, SignalSet, StatusQuery, StoreStatus,
    SubmissionId, TelemetryClient,
};
#[doc(inline)]
pub use config::{
    ConfigOverrides, ConfigSources, DiskBoundPolicy, ExporterBackendId, FileOtlp, FileStore,
    Secret, SyncHttpRetryPolicyDto, TelemetryClientConfig, TelemetryFileConfig, resolve_config,
};
#[doc(inline)]
pub use envelope::{
    LogInput, MetricInput, ProfilesInput, ProfilesSubmission, SpanInput, SubmissionEnvelope,
    SubmissionInput,
};
#[doc(inline)]
pub use errors::{
    AdmissionError, DeliveryError, SubmissionError, TelemetryClientError, TelemetryConfigError,
};

/// Scripted client and reusable lifecycle conformance tests.
#[cfg(feature = "test-double")]
pub mod testing;
