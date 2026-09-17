//! B.P3 direct-path and shared-core runtime-level fixture.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    reason = "integration fixture keeps one process-global bridge installation"
)]

use std::time::Duration;

use sc_observability_log::{
    ActionName, AdmissionOutcome, BridgeEvent, BridgeOptions, DropCause, EmitError, FieldKeyError,
    LevelChange, LevelChangeSource, LevelFilter, LoggerConfig, ServiceName, TargetCategory,
    WaitError,
};

struct Scratch(std::path::PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn direct_facade_and_macro_admission_share_the_core_level_owner() {
    let scratch = Scratch(std::env::temp_dir().join(format!(
        "sc-observability-log-bp3-runtime-{}",
        std::process::id()
    )));
    let mut config =
        LoggerConfig::default_for(ServiceName::new("bp3-runtime").unwrap(), scratch.0.clone());
    config.level = LevelFilter::Info;
    config.enable_console_sink = false;
    let mut guard = sc_observability_log::init(
        config,
        BridgeOptions {
            default_action: ActionName::new("log.record").unwrap(),
            parse_bracket_action: false,
        },
    )
    .unwrap();
    let control = guard.control();
    let path = control.active_log_path().unwrap().unwrap();
    assert!(
        matches!(
            control.wait_stopped(Duration::ZERO),
            Err(WaitError::NotStarted)
        ),
        "a control observes but never starts owner shutdown"
    );
    let direct = || BridgeEvent {
        level: sc_observability_log::EventLevel::Debug,
        target: TargetCategory::new("bp3.direct").unwrap(),
        action: Some(ActionName::new("runtime.level").unwrap()),
        message: Some("direct debug event".to_owned()),
        outcome: None,
        fields: serde_json::Map::new(),
        request_id: None,
        correlation_id: None,
        trace: None,
    };

    let mut colliding_fields = serde_json::Map::new();
    colliding_fields.insert("bp3.field".to_owned(), serde_json::json!(1));
    colliding_fields.insert("bp3::field".to_owned(), serde_json::json!(2));
    let before_collision = control.dropped_events().get(DropCause::InvalidEvent);
    assert!(matches!(
        control.try_log(BridgeEvent {
            fields: colliding_fields,
            ..direct()
        }),
        Err(EmitError::InvalidField {
            raw_key,
            reason: FieldKeyError::Collision { other_raw_key },
        }) if raw_key == "bp3::field" && other_raw_key == "bp3.field"
    ));
    assert_eq!(
        control.dropped_events().get(DropCause::InvalidEvent),
        before_collision + 1,
        "a rejected direct collision is accounted exactly once"
    );

    assert_eq!(
        control.try_log(direct()).unwrap(),
        AdmissionOutcome::Filtered
    );
    assert!(matches!(
        guard
            .elevate_level(LevelFilter::Debug, LevelChangeSource::DiagnosticSession)
            .unwrap(),
        LevelChange::Changed { .. }
    ));
    assert_eq!(
        control.try_log(direct()).unwrap(),
        AdmissionOutcome::Accepted
    );
    sc_observability_log::debug!(target: "bp3.macro", "macro debug event");
    log::debug!(target: "bp3.facade", "facade debug event");
    let health = guard.health().unwrap();
    assert_eq!(health.configured_level, LevelFilter::Info);
    assert_eq!(health.effective_level, LevelFilter::Debug);
    assert!(health.level_revision >= 1);
    assert!(matches!(
        guard
            .elevate_level(LevelFilter::Trace, LevelChangeSource::DiagnosticSession)
            .unwrap(),
        LevelChange::Changed { .. }
    ));
    assert_eq!(
        control
            .try_log(BridgeEvent {
                level: sc_observability_log::EventLevel::Trace,
                message: Some("direct trace event".to_owned()),
                ..direct()
            })
            .unwrap(),
        AdmissionOutcome::Accepted
    );
    sc_observability_log::trace!(target: "bp3.macro", "macro trace event");
    log::trace!(target: "bp3.facade", "facade trace event");
    guard.flush(Duration::from_secs(5)).unwrap();
    let contents = std::fs::read_to_string(&path).unwrap();
    for expected in [
        "direct debug event",
        "macro debug event",
        "facade debug event",
        "direct trace event",
        "macro trace event",
        "facade trace event",
    ] {
        assert!(
            contents.contains(expected),
            "missing {expected:?}: {contents}"
        );
    }
    guard.shutdown(Duration::from_secs(5)).unwrap();
    let stopped_health = control.health().unwrap();
    assert_eq!(stopped_health.configured_level, LevelFilter::Info);
    assert_eq!(stopped_health.effective_level, LevelFilter::Trace);
    assert!(stopped_health.level_revision >= 2);
    let before_post_stop = control.dropped_events().get(DropCause::NotInstalled);
    assert!(matches!(
        control.try_log(direct()),
        Err(EmitError::NotRunning { .. })
    ));
    assert_eq!(
        control.dropped_events().get(DropCause::NotInstalled),
        before_post_stop + 1,
        "post-stop direct rejection is accounted exactly once"
    );
    let first_report = control.wait_stopped(Duration::from_secs(1)).unwrap();
    assert!(matches!(
        first_report.outcome,
        sc_observability_log::ShutdownOutcome::Stopped
    ));
    let second_report = control.wait_stopped(Duration::ZERO).unwrap();
    assert_eq!(
        serde_json::to_value(&first_report).unwrap(),
        serde_json::to_value(&second_report).unwrap(),
        "all controls receive the one saved shutdown result"
    );
}
