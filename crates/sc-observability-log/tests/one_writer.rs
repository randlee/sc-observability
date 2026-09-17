//! Cross-producer direct-admission evidence for the single guarded writer.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "integration fixture owns one process-global bridge"
)]

use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::Duration;

use sc_observability::{RedactionPolicy, Redactor};
use sc_observability_log::{
    ActionName, AdmissionOutcome, BridgeEvent, BridgeOptions, DropCause, EmitError, EventLevel,
    FieldKeyError, LevelFilter, LogControl, LoggerConfig, ServiceName, TargetCategory,
};
use serde_json::Value;

static CONTROL: OnceLock<LogControl> = OnceLock::new();
static NESTED: Mutex<Vec<Result<AdmissionOutcome, EmitError>>> = Mutex::new(Vec::new());

fn event(message: &str) -> BridgeEvent {
    BridgeEvent {
        level: EventLevel::Info,
        target: TargetCategory::new("one_writer").unwrap(),
        action: None,
        message: Some(message.to_owned()),
        outcome: None,
        fields: serde_json::Map::new(),
        request_id: None,
        correlation_id: None,
        trace: None,
    }
}

struct ReenteringRedactor;

impl Redactor for ReenteringRedactor {
    fn redact(&self, key: &str, _value: &mut serde_json::Value) {
        match key {
            "nest_facade" => log::info!(target: "one_writer", "nested facade record"),
            "nest_direct" => NESTED.lock().unwrap_or_else(PoisonError::into_inner).push(
                CONTROL
                    .get()
                    .unwrap()
                    .try_log(event("nested direct record")),
            ),
            "panic" => panic!("expected fixture redactor panic"),
            _ => {}
        }
    }
}

fn counted_once(control: &LogControl, cause: DropCause, action: impl FnOnce()) {
    let before = control.health().unwrap().dropped;
    action();
    let after = control.health().unwrap().dropped;
    for other in DropCause::ALL {
        assert_eq!(
            after.get(other),
            before.get(other) + u64::from(other == cause)
        );
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one process-global fixture keeps cross-producer accounting deterministic"
)]
fn direct_facade_and_macro_share_guard_accounting_and_envelope() {
    let root = tempfile::tempdir().unwrap();
    let mut config = LoggerConfig::default_for(
        ServiceName::new("one-writer").unwrap(),
        root.path().to_path_buf(),
    );
    config.level = LevelFilter::Info;
    config.enable_console_sink = false;
    config.redaction = RedactionPolicy {
        custom_redactors: vec![Box::new(ReenteringRedactor)],
        ..RedactionPolicy::default()
    };
    let guard = sc_observability_log::init(
        config,
        BridgeOptions {
            default_action: ActionName::new("log.record").unwrap(),
            parse_bracket_action: false,
        },
    )
    .unwrap();
    let control = guard.control();
    CONTROL.set(control.clone()).unwrap();
    let path = control.active_log_path().unwrap().unwrap();

    assert_eq!(
        control.try_log(event("direct record")).unwrap(),
        AdmissionOutcome::Accepted
    );
    log::info!(target: "one_writer", "facade record");
    sc_observability_log::info!(target: "one_writer", "macro record");
    let filtered = BridgeEvent {
        level: EventLevel::Debug,
        ..event("filtered direct record")
    };
    assert_eq!(
        control.try_log(filtered).unwrap(),
        AdmissionOutcome::Filtered
    );

    let invalid = BridgeEvent {
        fields: serde_json::Map::from_iter([(
            "sc_observability_log.private".to_owned(),
            serde_json::json!(true),
        )]),
        ..event("invalid direct record")
    };
    counted_once(&control, DropCause::InvalidEvent, || {
        assert!(matches!(
            control.try_log(invalid),
            Err(EmitError::InvalidField {
                reason: FieldKeyError::ReservedPrefix,
                ..
            })
        ));
    });

    counted_once(&control, DropCause::ReentrantEmit, || {
        sc_observability_log::info!(target: "one_writer", nest_direct = true, "outer macro");
    });
    assert!(matches!(
        NESTED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_slice(),
        [Err(EmitError::Reentrant)]
    ));
    counted_once(&control, DropCause::ReentrantEmit, || {
        assert_eq!(
            control
                .try_log(BridgeEvent {
                    fields: serde_json::Map::from_iter([(
                        "nest_facade".to_owned(),
                        serde_json::json!(true),
                    )]),
                    ..event("outer direct")
                })
                .unwrap(),
            AdmissionOutcome::Accepted
        );
    });
    let old_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    counted_once(&control, DropCause::LoggerPanicked, || {
        assert!(matches!(
            control.try_log(BridgeEvent {
                fields: serde_json::Map::from_iter([
                    ("panic".to_owned(), serde_json::json!(true),)
                ]),
                ..event("panicking direct")
            }),
            Err(EmitError::Panicked)
        ));
    });
    counted_once(&control, DropCause::LoggerPanicked, || {
        log::info!(target: "one_writer", panic = true; "panicking facade");
    });
    counted_once(&control, DropCause::LoggerPanicked, || {
        sc_observability_log::info!(target: "one_writer", panic = true, "panicking macro");
    });
    std::panic::set_hook(old_hook);

    control.flush(Duration::from_secs(5)).unwrap();
    guard.shutdown(Duration::from_secs(5)).unwrap();
    counted_once(&control, DropCause::NotInstalled, || {
        assert!(matches!(
            control.try_log(event("post-stop direct")),
            Err(EmitError::NotRunning { .. })
        ));
    });
    let events: Vec<Value> = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let expected_messages = [
        "direct record",
        "facade record",
        "macro record",
        "outer macro",
        "outer direct",
    ];
    assert_eq!(events.len(), expected_messages.len());
    for message in expected_messages {
        let record = events
            .iter()
            .find(|event| event["message"] == message)
            .unwrap_or_else(|| panic!("missing {message:?}"));
        assert_eq!(record["level"], "Info");
        assert_eq!(record["service"], "one-writer");
        assert_eq!(record["target"], "one_writer");
        assert_eq!(record["action"], "log.record");
        assert_eq!(record["identity"]["pid"], u64::from(std::process::id()));
        assert!(record["version"].is_string());
        assert!(record["timestamp"].is_string());
    }
    for message in [
        "filtered direct record",
        "invalid direct record",
        "nested direct record",
        "nested facade record",
        "panicking direct",
        "panicking facade",
        "panicking macro",
        "post-stop direct",
    ] {
        assert!(
            events.iter().all(|event| event["message"] != message),
            "unexpected {message:?}"
        );
    }
    let jsonl_files: Vec<_> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|candidate| {
            candidate
                .extension()
                .is_some_and(|extension| extension == "jsonl")
        })
        .collect();
    assert_eq!(jsonl_files, vec![path]);
}
