use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

use sc_observability::{
    EnvSnapshot, LogSettings, LogSettingsError, LogSettingsInputs, RetainedLogPolicy,
};
use sc_observability_types::{EnvPrefix, LevelFilter, ServiceName};

fn snapshot(values: &[(&str, &str)]) -> EnvSnapshot {
    EnvSnapshot::from_pairs(
        values
            .iter()
            .map(|(key, value)| (OsString::from(*key), OsString::from(*value))),
    )
}

#[test]
fn parses_every_environment_inventory_row_atomically() {
    let settings = LogSettings::from_env(
        &snapshot(&[
            ("SC_LOG_LEVEL", "Debug"),
            ("SC_LOG_ROOT", "/shared"),
            ("SC_LOG_FILE", "false"),
            ("SC_LOG_CONSOLE", "true"),
            ("SC_LOG_ROTATION_MAX_BYTES", "1024"),
            ("SC_LOG_ROTATION_MAX_FILES", "3"),
            ("SC_LOG_RETENTION_MAX_AGE_MS", "60000"),
            ("SC_LOG_MAINTENANCE_CADENCE_MS", "1000"),
            ("SC_LOG_WRITER_SHUTDOWN_TIMEOUT_MS", "2000"),
            ("SC_LOG_MAINTENANCE_MAX_WORK_PER_PASS", "7"),
        ]),
        EnvPrefix::new("SC").expect("valid prefix"),
    )
    .expect("inventory parses");
    assert_eq!(settings.level, Some(LevelFilter::Debug));
    assert_eq!(settings.log_root, Some(PathBuf::from("/shared")));
    assert_eq!(settings.enable_file_sink, Some(false));
    assert_eq!(settings.enable_console_sink, Some(true));
    let policy = settings
        .retained_log_policy
        .expect("atomic policy override");
    assert_eq!(policy.rotation_max_bytes.as_u64(), 1024);
    assert_eq!(policy.rotation_max_files.as_usize(), 3);
    assert_eq!(policy.retention_max_age.as_duration().as_millis(), 60_000);
    assert_eq!(policy.maintenance_cadence.as_duration().as_millis(), 1_000);
    assert_eq!(
        policy.writer_shutdown_timeout.as_duration().as_millis(),
        2_000
    );
    assert_eq!(policy.maintenance_max_work_per_pass, Some(7));
}

#[test]
fn precedence_root_exception_and_conversion_preserve_defaults() {
    let file: LogSettings = serde_json::from_str(
        r#"{"level":"Warn","logRoot":"/json","enableFileSink":false,"retainedLogPolicy":{"rotation_max_bytes":12,"rotation_max_files":2,"retention_max_age":1000,"maintenance_cadence":100,"writer_shutdown_timeout":100,"maintenance_max_work_per_pass":null}}"#,
    )
    .expect("json settings");
    let shared = LogSettings::from_env(
        &snapshot(&[("SC_LOG_LEVEL", "Info"), ("SC_LOG_ROOT", "/shared")]),
        EnvPrefix::new("SC").unwrap(),
    )
    .unwrap();
    let application = LogSettings::from_env(
        &snapshot(&[("APP_LOG_LEVEL", "Error"), ("APP_LOG_ROOT", "/app")]),
        EnvPrefix::new("APP").unwrap(),
    )
    .unwrap();
    let resolved = LogSettings::resolve(LogSettingsInputs {
        file: Some(file),
        shared_env: shared,
        application_env: Some(application),
        default_root: PathBuf::from("/default"),
    })
    .unwrap();
    assert_eq!(resolved.level, LevelFilter::Error);
    assert_eq!(resolved.log_root.as_ref(), std::path::Path::new("/json"));
    let config = resolved.into_logger_config(ServiceName::new("settings-test").unwrap());
    assert_eq!(config.level, LevelFilter::Error);
    assert_eq!(
        config.queue_capacity,
        sc_observability::constants::DEFAULT_LOG_QUEUE_CAPACITY
    );
    assert!(config.redaction.redact_bearer_tokens);
}

#[test]
fn rejects_empty_unknown_case_and_prefix_collision() {
    let empty = LogSettings::from_env(
        &snapshot(&[("SC_LOG_ROOT", "")]),
        EnvPrefix::new("SC").unwrap(),
    )
    .unwrap_err();
    assert_eq!(empty.code().as_str(), "SC_LOG_SETTINGS_INVALID_VALUE");
    let unknown = LogSettings::from_env(
        &snapshot(&[("SC_LOG_UNKNOWN", "x")]),
        EnvPrefix::new("SC").unwrap(),
    )
    .unwrap_err();
    assert_eq!(unknown.code().as_str(), "SC_LOG_SETTINGS_UNKNOWN_KEY");
    let case = LogSettings::from_env(
        &snapshot(&[("sc_log_level", "Info")]),
        EnvPrefix::new("SC").unwrap(),
    )
    .unwrap_err();
    assert_eq!(case.code().as_str(), "SC_LOG_SETTINGS_INVALID_ENVIRONMENT");
    let collision =
        LogSettings::from_application_env(&snapshot(&[]), EnvPrefix::new("SC").unwrap())
            .unwrap_err();
    assert!(matches!(
        collision,
        LogSettingsError::PrefixCollision { .. }
    ));
}

#[cfg(unix)]
#[test]
fn rejects_non_utf8_key_in_selected_namespace() {
    let snapshot = EnvSnapshot::from_pairs([(
        OsString::from_vec(b"SC_LOG_\xFF".to_vec()),
        OsString::from("ignored"),
    )]);

    let error = LogSettings::from_env(&snapshot, EnvPrefix::new("SC").unwrap()).unwrap_err();
    assert_eq!(error.code().as_str(), "SC_LOG_SETTINGS_INVALID_ENVIRONMENT");
}

#[test]
fn empty_json_root_is_never_overridden_and_json_null_is_unset() {
    let file: LogSettings = serde_json::from_str(r#"{"logRoot":""}"#).unwrap();
    let error = LogSettings::resolve(LogSettingsInputs {
        file: Some(file),
        shared_env: LogSettings::default(),
        application_env: None,
        default_root: PathBuf::from("/default"),
    })
    .unwrap_err();
    assert_eq!(error.code().as_str(), "SC_LOG_SETTINGS_INVALID_VALUE");
    let null: LogSettings =
        serde_json::from_str(r#"{"level":null,"retainedLogPolicy":null}"#).unwrap();
    let resolved = LogSettings::resolve(LogSettingsInputs {
        file: Some(null),
        shared_env: LogSettings::default(),
        application_env: None,
        default_root: PathBuf::from("/default"),
    })
    .unwrap();
    assert_eq!(resolved.level, LevelFilter::Info);
    assert_eq!(resolved.retained_log_policy, RetainedLogPolicy::default());
}
