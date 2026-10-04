use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tonic::{Code, Status};

fn status_with(code: Code, duration: Option<(i64, i32)>) -> Status {
    let info = RetryInfo {
        retry_delay: duration.map(|(seconds, nanos)| WireDuration { seconds, nanos }),
    };
    let status = RpcStatus {
        details: vec![Detail {
            type_url: "type.googleapis.com/google.rpc.RetryInfo".into(),
            value: info.encode_to_vec(),
        }],
    };
    Status::with_details(code, "collector pacing", status.encode_to_vec().into())
}

#[test]
fn every_grpc_code_has_the_pinned_policy() {
    for code in [
        Code::Ok,
        Code::Cancelled,
        Code::Unknown,
        Code::InvalidArgument,
        Code::DeadlineExceeded,
        Code::NotFound,
        Code::AlreadyExists,
        Code::PermissionDenied,
        Code::ResourceExhausted,
        Code::FailedPrecondition,
        Code::Aborted,
        Code::OutOfRange,
        Code::Unimplemented,
        Code::Internal,
        Code::Unavailable,
        Code::DataLoss,
        Code::Unauthenticated,
    ] {
        let expected = match code {
            Code::Cancelled
            | Code::DeadlineExceeded
            | Code::Aborted
            | Code::OutOfRange
            | Code::Unavailable
            | Code::DataLoss => RetryDecision::Retryable,
            _ => RetryDecision::Terminal,
        };
        assert_eq!(
            classify_grpc(&Status::new(code, "fixture")),
            expected,
            "{code:?}"
        );
    }
}

#[test]
fn retry_info_validates_wire_values_and_conditional_throttling() {
    for duration in [
        None,
        Some((-1, 0)),
        Some((0, -1)),
        Some((1, 1_000_000_000)),
        Some((i64::MAX, 0)),
    ] {
        assert_eq!(
            classify_grpc(&status_with(Code::ResourceExhausted, duration)),
            RetryDecision::Terminal
        );
        assert_eq!(
            classify_grpc(&status_with(Code::Unavailable, duration)),
            RetryDecision::Retryable
        );
    }
    assert_eq!(
        classify_grpc(&status_with(Code::ResourceExhausted, Some((0, 0)))),
        RetryDecision::Throttled(Duration::ZERO)
    );
    assert_eq!(
        classify_grpc(&status_with(Code::Unavailable, Some((0, 0)))),
        RetryDecision::Retryable
    );
    for code in [Code::Unavailable, Code::ResourceExhausted] {
        assert_eq!(
            classify_grpc(&status_with(code, Some((2, 42)))),
            RetryDecision::Throttled(Duration::new(2, 42))
        );
        assert_eq!(
            classify_grpc(&status_with(code, Some((601, 0)))),
            RetryDecision::Throttled(SDK_RETRY_INFO_CAP)
        );
    }
    for bytes in [
        vec![0xff],
        RpcStatus {
            details: vec![Detail {
                type_url: "unknown".into(),
                value: vec![0xff],
            }],
        }
        .encode_to_vec(),
        RpcStatus {
            details: vec![Detail {
                type_url: "type.googleapis.com/google.rpc.RetryInfo".into(),
                value: vec![0xff],
            }],
        }
        .encode_to_vec(),
    ] {
        assert_eq!(
            classify_grpc(&Status::with_details(
                Code::ResourceExhausted,
                "",
                bytes.into()
            )),
            RetryDecision::Terminal
        );
    }
    // Pacing on a permanent status never makes it retryable.
    assert_eq!(
        classify_grpc(&status_with(Code::Internal, Some((2, 0)))),
        RetryDecision::Terminal
    );
}

#[test]
fn throttle_seeds_backoff_and_jitter_is_bounded() {
    let mut backoff = Backoff::new();
    assert_eq!(
        backoff.next(
            RetryDecision::Throttled(Duration::from_secs(7)),
            Duration::ZERO
        ),
        Some(Duration::from_secs(7))
    );
    assert_eq!(
        backoff.next(RetryDecision::Retryable, Duration::ZERO),
        Some(Duration::from_secs(14))
    );
    assert_eq!(
        backoff.next(
            RetryDecision::Throttled(SDK_RETRY_INFO_CAP),
            Duration::from_secs(999)
        ),
        Some(SDK_THROTTLE_CAP)
    );
    assert_eq!(
        Backoff::new().next(RetryDecision::Retryable, Duration::from_secs(999)),
        Some(Duration::from_millis(
            DEFAULT_OTLP_INITIAL_BACKOFF_MS + SDK_RETRY_JITTER_MS
        ))
    );
}

#[tokio::test(start_paused = true)]
async fn server_delay_must_fit_budget_without_an_early_second_send() {
    let (_tx, rx) = watch::channel(false);
    let mut calls = 0;
    let started = Instant::now();
    let result = retry_with_jitter(
        Duration::from_secs(1),
        Duration::from_secs(1),
        rx,
        |_| {
            calls += 1;
            async { Err::<(), _>(status_with(Code::Unavailable, Some((2, 0)))) }
        },
        "fixture",
        || Duration::ZERO,
    )
    .await;
    assert!(result.is_err());
    assert_eq!(calls, 1);
    assert_eq!(Instant::now(), started);
}

#[tokio::test(start_paused = true)]
async fn hung_attempts_respect_request_and_absolute_deadline() {
    let (_tx, rx) = watch::channel(false);
    let mut timeouts = Vec::new();
    let started = Instant::now();
    let result = retry_with_jitter(
        Duration::from_secs(1),
        Duration::from_millis(300),
        rx,
        |timeout| {
            timeouts.push(timeout);
            std::future::pending::<Result<(), Status>>()
        },
        "fixture",
        || Duration::ZERO,
    )
    .await;
    assert!(result.is_err());
    assert_eq!(
        timeouts,
        vec![Duration::from_millis(300), Duration::from_millis(300)]
    );
    assert_eq!(Instant::now() - started, Duration::from_millis(850));
    // A request budget longer than the sequence is clamped, not reset.
    let (_tx, rx) = watch::channel(false);
    let started = Instant::now();
    let result = retry_with_jitter(
        Duration::from_millis(100),
        Duration::from_secs(2),
        rx,
        |timeout| {
            assert_eq!(timeout, Duration::from_millis(100));
            std::future::pending::<Result<(), Status>>()
        },
        "fixture",
        || Duration::ZERO,
    )
    .await;
    assert!(result.is_err());
    assert_eq!(Instant::now() - started, Duration::from_millis(100));
}

#[tokio::test(start_paused = true)]
async fn overslept_timer_does_not_launch_another_rpc_and_preserves_cause() {
    let (_tx, rx) = watch::channel(false);
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let (wait_tx, wait_rx) = tokio::sync::oneshot::channel();
    let mut wait_tx = Some(wait_tx);
    let task = tokio::spawn(retry_with_jitter(
        Duration::from_secs(1),
        Duration::from_secs(1),
        rx,
        move |_| {
            observed.fetch_add(1, Ordering::SeqCst);
            async { Err::<(), _>(Status::unavailable("original cause")) }
        },
        "fixture",
        move || {
            if let Some(tx) = wait_tx.take() {
                let _ = tx.send(());
            }
            Duration::ZERO
        },
    ));
    wait_rx.await.unwrap();
    tokio::time::advance(Duration::from_secs(2)).await;
    let error = task.await.unwrap().unwrap_err();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(&error);
    let mut found = false;
    while let Some(error) = source {
        if let Some(status) = error.downcast_ref::<Status>() {
            assert_eq!(status.message(), "original cause");
            found = true;
        }
        source = error.source();
    }
    assert!(found);
}

#[tokio::test(start_paused = true)]
async fn http_hung_attempt_uses_same_absolute_deadline() {
    let (_tx, rx) = watch::channel(false);
    let started = Instant::now();
    assert!(
        retry_with_jitter(
            Duration::from_millis(90),
            Duration::from_secs(5),
            rx,
            |timeout| {
                assert_eq!(timeout, Duration::from_millis(90));
                std::future::pending::<Result<(), HttpFailure>>()
            },
            "HTTP fixture",
            || Duration::ZERO
        )
        .await
        .is_err()
    );
    assert_eq!(Instant::now() - started, Duration::from_millis(90));
}

#[tokio::test(start_paused = true)]
async fn exhausted_http_timeout_preserves_its_source() {
    // Use the existing classified error as the fake operation result; the SDK
    // loopback test separately covers real reqwest -> HttpError classification.
    let (_tx, rx) = watch::channel(false);
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let (wait_tx, wait_rx) = tokio::sync::oneshot::channel();
    let mut wait_tx = Some(wait_tx);
    let task = tokio::spawn(retry_with_jitter(
        Duration::from_secs(1),
        Duration::from_secs(1),
        rx,
        move |_| {
            observed.fetch_add(1, Ordering::SeqCst);
            async { Err::<(), _>(HttpFailure::ConnectOrTimeout) }
        },
        "HTTP timeout fixture",
        move || {
            if let Some(tx) = wait_tx.take() {
                let _ = tx.send(());
            }
            Duration::ZERO
        },
    ));
    wait_rx.await.unwrap();
    tokio::time::advance(Duration::from_secs(2)).await;
    let error = task.await.unwrap().unwrap_err();
    assert_eq!(error.code().to_string(), "OTLP_EXPORT_TERMINAL");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let mut source = std::error::Error::source(&error);
    let mut found = None;
    while let Some(error) = source {
        if let Some(failure) = error.downcast_ref::<HttpFailure>() {
            found = Some(*failure);
        }
        source = error.source();
    }
    assert_eq!(found, Some(HttpFailure::ConnectOrTimeout));
}
