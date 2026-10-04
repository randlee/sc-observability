//! Private retry policy for the lossless protobuf adapter, pinned to OTLP 0.33.
#[cfg(test)]
use super::implementation::HttpFailure;
use super::implementation::{RetryClass, shutdown_cancelled_error, transport_error_from};
use crate::constants::{
    DEFAULT_OTLP_INITIAL_BACKOFF_MS, DEFAULT_OTLP_MAX_BACKOFF_MS, DEFAULT_OTLP_MAX_RETRIES,
    PROTO_DURATION_MAX_SECONDS, SDK_RETRY_INFO_CAP, SDK_RETRY_JITTER_MS, SDK_THROTTLE_CAP,
};
use prost::Message;
use sc_observability_types::v2::ExportError;
use std::{collections::hash_map::RandomState, future::Future, hash::BuildHasher, time::Duration};
use tokio::{
    sync::watch,
    time::{Instant, sleep, timeout_at},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RetryDecision {
    Terminal,
    Retryable,
    Throttled(Duration),
}

// Only the wire fields required to read google.rpc.RetryInfo. prost safely skips
// other fields; no copy of the SDK's exporter or new dependency is necessary.
#[derive(Clone, PartialEq, Message)]
struct RpcStatus {
    #[prost(message, repeated, tag = "3")]
    details: Vec<Detail>,
}
#[derive(Clone, PartialEq, Message)]
struct Detail {
    #[prost(string, tag = "1")]
    type_url: String,
    #[prost(bytes = "vec", tag = "2")]
    value: Vec<u8>,
}
#[derive(Clone, PartialEq, Message)]
struct RetryInfo {
    #[prost(message, optional, tag = "1")]
    retry_delay: Option<WireDuration>,
}
#[derive(Clone, PartialEq, Message)]
struct WireDuration {
    #[prost(int64, tag = "1")]
    seconds: i64,
    #[prost(int32, tag = "2")]
    nanos: i32,
}

fn retry_info(status: &tonic::Status) -> Option<Duration> {
    let details = RpcStatus::decode(status.details()).ok()?;
    for detail in details.details {
        if detail.type_url != "type.googleapis.com/google.rpc.RetryInfo" {
            continue;
        }
        let delay = RetryInfo::decode(detail.value.as_slice())
            .ok()?
            .retry_delay?;
        if !(0..=PROTO_DURATION_MAX_SECONDS).contains(&delay.seconds)
            || !(0..1_000_000_000).contains(&delay.nanos)
        {
            return None;
        }
        return Some(
            Duration::new(
                u64::try_from(delay.seconds).ok()?,
                u32::try_from(delay.nanos).ok()?,
            )
            .min(SDK_RETRY_INFO_CAP),
        );
    }
    None
}

pub(super) fn classify_grpc(status: &tonic::Status) -> RetryDecision {
    use tonic::Code;
    match status.code() {
        Code::ResourceExhausted => {
            retry_info(status).map_or(RetryDecision::Terminal, RetryDecision::Throttled)
        }
        Code::Unavailable => retry_info(status)
            .filter(|delay| !delay.is_zero())
            .map_or(RetryDecision::Retryable, RetryDecision::Throttled),
        Code::Cancelled
        | Code::DeadlineExceeded
        | Code::Aborted
        | Code::OutOfRange
        | Code::DataLoss => RetryDecision::Retryable,
        Code::Ok
        | Code::Unknown
        | Code::InvalidArgument
        | Code::NotFound
        | Code::AlreadyExists
        | Code::PermissionDenied
        | Code::FailedPrecondition
        | Code::Unimplemented
        | Code::Internal
        | Code::Unauthenticated => RetryDecision::Terminal,
    }
}

struct Backoff {
    delay: Duration,
    cap: Duration,
}
impl Backoff {
    fn new() -> Self {
        Self {
            delay: Duration::from_millis(DEFAULT_OTLP_INITIAL_BACKOFF_MS),
            cap: Duration::from_millis(DEFAULT_OTLP_MAX_BACKOFF_MS),
        }
    }
    fn next(&mut self, decision: RetryDecision, jitter: Duration) -> Option<Duration> {
        match decision {
            RetryDecision::Terminal => return None,
            RetryDecision::Retryable => {}
            RetryDecision::Throttled(server) => {
                let server = server.min(SDK_THROTTLE_CAP);
                if server > self.cap {
                    self.cap = SDK_THROTTLE_CAP;
                }
                self.delay = self.delay.max(server);
            }
        }
        let wait = self
            .delay
            .saturating_add(jitter.min(Duration::from_millis(SDK_RETRY_JITTER_MS)))
            .min(self.cap);
        self.delay = self.delay.saturating_mul(2).min(self.cap);
        Some(wait)
    }
}

pub(super) async fn retry_export_bounded<F, Fut, E>(
    budget: Duration,
    request_timeout: Duration,
    shutdown: watch::Receiver<bool>,
    operation: F,
    message: &'static str,
) -> Result<(), ExportError>
where
    F: FnMut(Duration) -> Fut + Send,
    Fut: Future<Output = Result<(), E>> + Send,
    E: RetryClass + std::error::Error + Send + Sync + 'static,
{
    let random = RandomState::new();
    let mut sample = 0_u64;
    retry_with_jitter(
        budget,
        request_timeout,
        shutdown,
        operation,
        message,
        move || {
            sample = sample.wrapping_add(1);
            Duration::from_millis(random.hash_one(sample) % (SDK_RETRY_JITTER_MS + 1))
        },
    )
    .await
}

/// Tokio's clock is injectable via its paused-time runtime. Jitter is an explicit
/// input so tests exercise the production executor without wall-clock waits.
pub(super) async fn retry_with_jitter<F, Fut, E, J>(
    budget: Duration,
    request_timeout: Duration,
    mut shutdown: watch::Receiver<bool>,
    mut operation: F,
    message: &'static str,
    mut jitter: J,
) -> Result<(), ExportError>
where
    F: FnMut(Duration) -> Fut + Send,
    Fut: Future<Output = Result<(), E>> + Send,
    E: RetryClass + std::error::Error + Send + Sync + 'static,
    J: FnMut() -> Duration + Send,
{
    let deadline = Instant::now() + budget;
    let mut backoff = Backoff::new();
    let mut retries = 0;
    let mut last_error: Option<Box<dyn std::error::Error + Send + Sync>> = None;
    loop {
        if *shutdown.borrow() {
            return Err(shutdown_cancelled_error());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() || request_timeout.is_zero() {
            return Err(transport_error_from(
                message,
                last_error.unwrap_or_else(|| {
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "OTLP retry sequence deadline exhausted",
                    ))
                }),
            ));
        }
        let attempt_timeout = request_timeout.min(remaining);
        let attempt_deadline = Instant::now() + attempt_timeout;
        let result = tokio::select! {
            biased;
            _ = shutdown.changed() => return Err(shutdown_cancelled_error()),
            result = timeout_at(attempt_deadline.min(deadline), operation(attempt_timeout)) => result,
        };
        let decision = match result {
            Ok(Ok(())) if Instant::now() < attempt_deadline.min(deadline) => return Ok(()),
            Ok(Ok(())) | Err(_) => {
                // Keep the last real protocol error if a later attempt times out.
                if last_error.is_none() {
                    last_error = Some(Box::new(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "OTLP request deadline exhausted",
                    )));
                }
                RetryDecision::Retryable
            }
            Ok(Err(error)) => {
                let decision = error.retry_decision();
                last_error = Some(Box::new(error));
                decision
            }
        };
        let remaining = deadline.saturating_duration_since(Instant::now());
        let wait = if retries < DEFAULT_OTLP_MAX_RETRIES && !remaining.is_zero() {
            backoff
                .next(decision, jitter())
                .filter(|wait| *wait < remaining)
        } else {
            None
        };
        let Some(wait) = wait else {
            return Err(transport_error_from(
                message,
                last_error.unwrap_or_else(|| {
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "OTLP retry sequence deadline exhausted",
                    ))
                }),
            ));
        };
        tokio::select! {
            biased;
            _ = shutdown.changed() => return Err(shutdown_cancelled_error()),
            () = sleep(wait) => {},
        }
        retries += 1;
        // Recheck absolute time after waking: scheduler delay never resets budget.
    }
}

// Existing unit fixtures use the same executor with a deterministic jitter source.
#[cfg(test)]
pub(super) async fn retry_export<F, Fut, E>(
    budget: Duration,
    shutdown: watch::Receiver<bool>,
    mut operation: F,
    message: &'static str,
) -> Result<(), ExportError>
where
    F: FnMut() -> Fut + Send,
    Fut: Future<Output = Result<(), E>> + Send,
    E: RetryClass + std::error::Error + Send + Sync + 'static,
{
    retry_with_jitter(
        budget,
        budget,
        shutdown,
        |_| operation(),
        message,
        || Duration::ZERO,
    )
    .await
}
#[cfg(test)]
pub(super) fn retry_action(
    code: tonic::Code,
    attempt: u32,
    elapsed: Duration,
    deadline: Duration,
    delay: Duration,
) -> Option<Duration> {
    action(
        classify_grpc(&tonic::Status::new(code, "")),
        attempt,
        elapsed,
        deadline,
        delay,
    )
}
#[cfg(test)]
pub(super) fn http_retry_action(
    failure: HttpFailure,
    attempt: u32,
    elapsed: Duration,
    deadline: Duration,
    delay: Duration,
) -> Option<Duration> {
    action(failure.retry_decision(), attempt, elapsed, deadline, delay)
}
#[cfg(test)]
fn action(
    decision: RetryDecision,
    attempt: u32,
    elapsed: Duration,
    deadline: Duration,
    delay: Duration,
) -> Option<Duration> {
    if attempt >= DEFAULT_OTLP_MAX_RETRIES {
        return None;
    }
    let mut backoff = Backoff {
        delay,
        cap: Duration::from_millis(DEFAULT_OTLP_MAX_BACKOFF_MS),
    };
    backoff
        .next(decision, Duration::ZERO)
        .filter(|wait| !wait.is_zero() && *wait < deadline.saturating_sub(elapsed))
}

#[cfg(test)]
mod tests;
