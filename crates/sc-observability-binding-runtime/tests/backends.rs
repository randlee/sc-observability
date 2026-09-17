use sc_observability_binding_runtime::{
    HostLoggingBackend, OperationState, ProducerOrigin, bridge_backend, create_core_backend,
};
use sc_observability_dto::{self as dto, AdmissionDto, CompletionDto, Failure};
use sc_observability_types::{LevelChangeSource, LevelFilter, ProcessIdentityPolicy, ServiceName};
use std::time::Duration;

fn config(root: &std::path::Path) -> sc_observability::LoggerConfig {
    let mut value = sc_observability::LoggerConfig::default_for(
        ServiceName::new("native-fixture").unwrap(),
        root.into(),
    );
    value.enable_console_sink = false;
    value.process_identity = ProcessIdentityPolicy::Fixed {
        hostname: Some("trusted-host".into()),
        pid: Some(42),
    };
    value
}
fn event() -> dto::LogEventDto {
    dto::decode_event(serde_json::json!({"schema_version":1,"level":"info","target":"native.fixture","action":"test.emit","fields":{"value":{"kind":"integer","value":"18446744073709551615"}}})).unwrap()
}
fn query() -> dto::LogQueryDto {
    dto::decode_query(serde_json::json!({"schema_version":1})).unwrap()
}
fn exercise(backend: &impl HostLoggingBackend) {
    assert_eq!(
        backend.try_log(event(), ProducerOrigin::Python).unwrap(),
        AdmissionDto::Accepted
    );
    assert_eq!(
        backend
            .start_flush(Duration::from_secs(2))
            .unwrap()
            .wait(Duration::from_secs(2))
            .unwrap(),
        CompletionDto::Completed
    );
    let snapshot = backend
        .start_query(query())
        .unwrap()
        .wait(Duration::from_secs(2))
        .unwrap();
    let record = snapshot
        .events
        .iter()
        .find(|event| event.action == "test.emit")
        .unwrap();
    assert_eq!(record.identity.hostname.as_deref(), Some("trusted-host"));
    assert_eq!(record.identity.pid, Some(42));
    assert_eq!(
        record.fields.get("sc_observability.binding.language"),
        Some(&dto::ValueDto::String {
            value: "python".into()
        })
    );
    assert_eq!(
        record.fields.get("sc_observability.binding.channel"),
        Some(&dto::ValueDto::String {
            value: "pyo3".into()
        })
    );
    for key in [
        "sc_observability.binding.language",
        "sc_observability.binding.other",
        "sc_observability::binding::language",
    ] {
        let mut forged = event();
        forged.fields.insert(
            key.into(),
            dto::ValueDto::String {
                value: "rust".into(),
            },
        );
        assert!(matches!(
            backend.try_log(forged, ProducerOrigin::RustHost),
            Err(Failure::Validation { .. })
        ));
    }
}
#[test]
fn core_roundtrip_and_surviving_read_handles() {
    let temp = tempfile::tempdir().unwrap();
    let (mut owner, backend) = create_core_backend(config(temp.path())).unwrap();
    exercise(&backend);
    owner
        .elevate_level(LevelFilter::Debug, LevelChangeSource::Application)
        .unwrap();
    owner.reset_level(LevelChangeSource::Application).unwrap();
    let first = owner.start_shutdown().unwrap();
    let second = owner.start_shutdown().unwrap();
    first.wait(Duration::from_secs(2)).unwrap();
    second.wait(Duration::ZERO).unwrap();
    assert!(matches!(
        first.state(),
        OperationState::Completed { result: Ok(_) }
    ));
    assert!(matches!(
        backend.try_log(event(), ProducerOrigin::RustHost),
        Err(Failure::Closed { .. })
    ));
    assert!(backend.health().is_ok());
}
#[test]
fn bridge_roundtrip_preserves_host_ownership() {
    let temp = tempfile::tempdir().unwrap();
    let host = sc_observability_log::init(
        config(temp.path()),
        sc_observability_log::BridgeOptions {
            default_action: sc_observability_types::ActionName::new("native.default").unwrap(),
            parse_bracket_action: false,
        },
    )
    .unwrap();
    let control = host.control();
    let backend = bridge_backend(control.clone()).unwrap();
    exercise(&backend);
    let snapshot = backend.health().unwrap();
    let bridge = snapshot.bridge.as_ref().unwrap();
    assert_eq!(snapshot.logging, bridge.logging);
    assert_eq!(snapshot.level_state.level_revision, bridge.level_revision);
    drop(backend);
    control.flush(Duration::from_secs(2)).unwrap();
    host.shutdown(Duration::from_secs(2)).unwrap();
}
