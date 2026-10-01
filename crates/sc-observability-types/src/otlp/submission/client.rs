//! Client lifecycle and observable admission/delivery state.
#![allow(
    clippy::result_large_err,
    reason = "approved client errors retain an inline four-signal FlushReport"
)]
use super::{SubmissionEnvelope, SubmissionError, TelemetryClientConfig, TelemetryClientError};
use crate::{ErrorCode, Timestamp};
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr, time::Duration};

/// Version of the durable envelope, independent from store schema version.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EnvelopeVersion(u16);
impl EnvelopeVersion {
    /// Current supported envelope format.
    pub const CURRENT: Self = Self(1);
    /// Retains an input version for explicit unsupported-version errors.
    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(value)
    }
    /// Numeric version.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}
/// Caller-stable local admission key, measured in UTF-8 bytes.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct RecordKey(String);
impl FromStr for RecordKey {
    type Err = SubmissionError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty() || value.len() > crate::constants::TELEMETRY_RECORD_KEY_MAX_BYTES {
            Err(SubmissionError::validation(
                "record_key",
                "key must contain 1..=256 UTF-8 bytes",
            ))
        } else {
            Ok(Self(value.into()))
        }
    }
}
impl TryFrom<String> for RecordKey {
    type Error = SubmissionError;
    fn try_from(v: String) -> Result<Self, Self::Error> {
        v.parse()
    }
}
impl fmt::Display for RecordKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}
/// `UUIDv7` assigned at admission.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct SubmissionId(String);
impl FromStr for SubmissionId {
    type Err = SubmissionError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let b = value.as_bytes();
        if b.len() != 36
            || !b.iter().enumerate().all(|(i, c)| {
                if [8, 13, 18, 23].contains(&i) {
                    *c == b'-'
                } else {
                    c.is_ascii_hexdigit()
                }
            })
            || b[14] != b'7'
            || !b"89abAB".contains(&b[19])
        {
            return Err(SubmissionError::validation(
                "submission_id",
                "expected a UUIDv7",
            ));
        }
        Ok(Self(value.to_ascii_lowercase()))
    }
}
impl TryFrom<String> for SubmissionId {
    type Error = SubmissionError;
    fn try_from(v: String) -> Result<Self, Self::Error> {
        v.parse()
    }
}
impl fmt::Display for SubmissionId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}
/// Independently delivered signal family.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Signal {
    /// Logs.
    Logs,
    /// Completed spans.
    Traces,
    /// Metric streams.
    Metrics,
    /// Profiles.
    Profiles,
}
impl Signal {
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Logs => 0,
            Self::Traces => 1,
            Self::Metrics => 2,
            Self::Profiles => 3,
        }
    }
}
/// Compact set of signal families.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SignalSet(u8);
impl SignalSet {
    /// Creates a set from the provided families.
    #[must_use]
    pub fn new(signals: impl IntoIterator<Item = Signal>) -> Self {
        Self(signals.into_iter().fold(0, |b, s| b | (1 << s.index())))
    }
    /// Whether this family is present.
    #[must_use]
    pub const fn contains(self, signal: Signal) -> bool {
        self.0 & (1 << signal.index()) != 0
    }
    /// Ordered families: logs, traces, metrics, profiles.
    pub fn iter(self) -> impl Iterator<Item = Signal> {
        [
            Signal::Logs,
            Signal::Traces,
            Signal::Metrics,
            Signal::Profiles,
        ]
        .into_iter()
        .filter(move |s| self.contains(*s))
    }
}
/// Signal representation requested from a backend.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Representation {
    /// Log record.
    Log,
    /// Span record.
    Span,
    /// Gauge points.
    Gauge,
    /// Sum points.
    Sum,
    /// Explicit histogram.
    Histogram,
    /// Exponential histogram.
    ExponentialHistogram,
    /// Summary points.
    Summary,
    /// Exemplars.
    Exemplar,
    /// Profile data.
    Profile,
}
/// Per-family row counts for the operation's scope.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalCounts {
    /// Log rows.
    pub logs: u64,
    /// Trace rows.
    pub traces: u64,
    /// Metric rows.
    pub metrics: u64,
    /// Profile rows.
    pub profiles: u64,
}
impl SignalCounts {
    /// Creates an empty count set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            logs: 0,
            traces: 0,
            metrics: 0,
            profiles: 0,
        }
    }
    /// Increments a signal count.
    pub fn increment(&mut self, signal: Signal) {
        match signal {
            Signal::Logs => self.logs += 1,
            Signal::Traces => self.traces += 1,
            Signal::Metrics => self.metrics += 1,
            Signal::Profiles => self.profiles += 1,
        }
    }
    /// Total rows in all signal families.
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.logs + self.traces + self.metrics + self.profiles
    }
}
/// Result counts limited to the rows in the flush scope.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlushReport {
    /// Delivered rows.
    pub delivered: SignalCounts,
    /// Nonterminal rows.
    pub still_pending: SignalCounts,
    /// Terminal failures.
    pub failed: SignalCounts,
    /// Explicit disk-bound evictions.
    pub evicted: SignalCounts,
}
impl FlushReport {
    /// Applies terminal-failure precedence over deadline expiry.
    /// # Errors
    /// Returns terminal failure when failed/evicted rows exist, otherwise deadline when pending rows remain.
    pub fn into_result(self) -> Result<Self, TelemetryClientError> {
        use super::{DeliveryError, error_codes, errors::context};
        if self.failed.total() > 0 || self.evicted.total() > 0 {
            return Err(DeliveryError::TerminalFailure {
                report: self,
                context: context(
                    error_codes::SC_OBSERVABILITY_DELIVERY_FAILED,
                    "one or more signal rows failed",
                ),
            }
            .into());
        }
        if self.still_pending.total() > 0 {
            return Err(DeliveryError::DeadlineExceeded {
                report: self,
                context: context(
                    error_codes::SC_OBSERVABILITY_DELIVERY_DEADLINE,
                    "signal rows remain pending",
                ),
            }
            .into());
        }
        Ok(self)
    }
}
/// Admission receipt; confirms local commitment, never remote delivery.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdmissionReceipt {
    /// Assigned `UUIDv7`.
    pub submission_id: SubmissionId,
    /// Optional caller key.
    pub record_key: Option<RecordKey>,
    /// UTC commit time.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::serialize")]
    pub admitted_at: Timestamp,
    /// Admitted signal families.
    pub signals: Vec<Signal>,
    /// Existing admission returned for an existing key.
    pub duplicate: bool,
}
/// Current lifecycle state of one admitted signal family.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DeliveryState {
    /// Awaiting delivery.
    Pending,
    /// Claimed by a drain owner.
    Claimed {
        /// Lease holder.
        holder: String,
        /// Export attempts.
        attempts: u32,
    },
    /// Waiting for a retry.
    RetryScheduled {
        /// Export attempts.
        attempts: u32,
        /// Next eligible time.
        #[serde(serialize_with = "crate::otlp::signals::timestamp::serialize")]
        next_attempt_at: Timestamp,
        /// Last export error.
        last_error: ErrorCode,
    },
    /// Collector accepted the signal.
    Delivered {
        /// Delivery time.
        #[serde(serialize_with = "crate::otlp::signals::timestamp::serialize")]
        at: Timestamp,
        /// Export attempts.
        attempts: u32,
    },
    /// Export failed terminally.
    Failed {
        /// Export attempts.
        attempts: u32,
        /// Terminal error.
        error: ErrorCode,
    },
    /// Evicted only under the explicitly selected disk policy.
    EvictedByDiskBound {
        /// Eviction time.
        #[serde(serialize_with = "crate::otlp::signals::timestamp::serialize")]
        at: Timestamp,
    },
}
impl DeliveryState {
    /// Whether the state ends delivery work for this row.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Delivered { .. } | Self::Failed { .. } | Self::EvictedByDiskBound { .. }
        )
    }
}
/// Per-signal status of an admitted submission.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeliveryStatus {
    /// Admission identity.
    pub submission_id: SubmissionId,
    /// Per-family states.
    pub signals: Vec<(Signal, DeliveryState)>,
}
/// Current drain lease owner and expiry.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeaseInfo {
    /// Process and UUID holder identity.
    pub holder: String,
    /// UTC lease expiry.
    #[serde(serialize_with = "crate::otlp::signals::timestamp::serialize")]
    pub expires_at: Timestamp,
}
/// Store summary with optionally selected submission details.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StoreStatus {
    /// Durable schema version.
    pub schema_version: u32,
    /// Current byte usage.
    pub store_bytes: u64,
    /// Configured byte bound.
    pub max_store_bytes: u64,
    /// Pending rows.
    pub pending: SignalCounts,
    /// Scheduled retries.
    pub retry_scheduled: SignalCounts,
    /// Retained delivered rows.
    pub delivered_retained: SignalCounts,
    /// Terminally failed rows.
    pub failed: SignalCounts,
    /// Cumulative evictions.
    pub evicted_by_disk_bound: u64,
    /// Cumulative capacity rejections.
    pub rejected_by_disk_bound: u64,
    /// Newer envelope rows left untouched.
    pub unreadable_newer_envelopes: u64,
    /// Current lease.
    pub lease: Option<LeaseInfo>,
    /// Selected submissions.
    pub submissions: Vec<DeliveryStatus>,
}
/// Scope of a status query.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum StatusQuery {
    /// Aggregate counters only.
    Summary,
    /// Selected admission identities.
    Submissions(Vec<SubmissionId>),
    /// Selected caller keys.
    RecordKeys(Vec<RecordKey>),
}
/// Runtime-independent durable telemetry lifecycle.
pub trait TelemetryClient: Send + Sync {
    /// Opens a client using resolved configuration.
    /// # Errors
    /// Returns configuration or store admission failures.
    fn open(config: TelemetryClientConfig) -> Result<Self, TelemetryClientError>
    where
        Self: Sized;
    /// Validates and commits an envelope before returning its receipt.
    /// # Errors
    /// Returns submission or admission failure without successful admission.
    fn emit(&self, envelope: SubmissionEnvelope) -> Result<AdmissionReceipt, TelemetryClientError>;
    /// Flushes rows nonterminal at the start of this call.
    /// # Errors
    /// Returns failure or deadline with scoped counts.
    fn flush(&self, deadline: Duration) -> Result<FlushReport, TelemetryClientError>;
    /// Flushes every row of one submission, including its historical terminal rows.
    /// # Errors
    /// Returns failure or deadline with scoped counts.
    fn flush_submission(
        &self,
        id: &SubmissionId,
        deadline: Duration,
    ) -> Result<FlushReport, TelemetryClientError>;
    /// Flushes and releases resources; repeated calls return an empty successful report.
    /// # Errors
    /// Returns the flush failure after releasing resources.
    fn shutdown(&self, deadline: Duration) -> Result<FlushReport, TelemetryClientError>;
    /// Reads stored state without claiming delivery success.
    /// # Errors
    /// Returns store failures.
    fn status(&self, query: StatusQuery) -> Result<StoreStatus, TelemetryClientError>;
}

impl AdmissionReceipt {
    /// Constructs the receipt after successful admission.
    #[must_use]
    pub fn new(
        submission_id: SubmissionId,
        record_key: Option<RecordKey>,
        admitted_at: Timestamp,
        signals: Vec<Signal>,
        duplicate: bool,
    ) -> Self {
        Self {
            submission_id,
            record_key,
            admitted_at,
            signals,
            duplicate,
        }
    }
}
impl DeliveryStatus {
    /// Associates signal states with an admission.
    #[must_use]
    pub fn new(submission_id: SubmissionId, signals: Vec<(Signal, DeliveryState)>) -> Self {
        Self {
            submission_id,
            signals,
        }
    }
}
impl LeaseInfo {
    /// Describes the current lease holder.
    #[must_use]
    pub fn new(holder: String, expires_at: Timestamp) -> Self {
        Self { holder, expires_at }
    }
}
