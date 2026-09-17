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
        "sc observability.binding.language",
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
        let mut nested = event();
        nested.fields.insert(
            "nested".into(),
            dto::ValueDto::Array {
                value: vec![dto::ValueDto::Object {
                    value: [(
                        key.into(),
                        dto::ValueDto::String {
                            value: "forged".into(),
                        },
                    )]
                    .into(),
                }],
            },
        );
        assert!(matches!(
            backend.try_log(nested, ProducerOrigin::TauriFrontend),
            Err(Failure::Validation { .. })
        ));
    }
}
fn core_roundtrip_and_surviving_read_handles() {
    let temp = Root::new();
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
fn bridge_roundtrip_preserves_host_ownership() {
    let temp = Root::new();
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

struct Root(std::path::PathBuf);
impl Root {
    fn new() -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("binding-consumer-{}-{unique}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn main() {
    core_roundtrip_and_surviving_read_handles();
    bridge_roundtrip_preserves_host_ownership();
    println!("BINDING_CONSUMER_OK runtime core+bridge provenance query flush health shutdown");
}
