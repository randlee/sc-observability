use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
#[cfg(windows)]
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use sc_observability::{
    EnvSnapshot, LogSettings, LogSettingsError, LogSettingsInputs, ResolvedLogSettings,
    RetainedLogPolicy, error_codes,
};
use sc_observability_types::{EnvPrefix, LevelFilter, Remediation, ServiceName};

fn snapshot(values: &[(&str, &str)]) -> EnvSnapshot {
    EnvSnapshot::from_pairs(
        values
            .iter()
            .map(|(key, value)| (OsString::from(*key), OsString::from(*value))),
    )
}

#[test]
fn settings_error_codes_match_documented_stable_names() {
    let codes = [
        (
            error_codes::LOG_PREFIX_COLLISION,
            "SC_LOG_SETTINGS_PREFIX_COLLISION",
        ),
        (
            error_codes::LOG_INVALID_ENVIRONMENT,
            "SC_LOG_SETTINGS_INVALID_ENVIRONMENT",
        ),
        (error_codes::LOG_UNKNOWN_KEY, "SC_LOG_SETTINGS_UNKNOWN_KEY"),
        (
            error_codes::LOG_INVALID_VALUE,
            "SC_LOG_SETTINGS_INVALID_VALUE",
        ),
        (error_codes::LOG_RESOLUTION, "SC_LOG_SETTINGS_RESOLUTION"),
    ];

    for (code, expected) in codes {
        assert_eq!(code.as_str(), expected);
    }
}

#[test]
fn settings_errors_include_actionable_recovery_steps_and_docs() {
    let invalid_value = LogSettings::from_env(
        &snapshot(&[("SC_LOG_FILE", "yes")]),
        &EnvPrefix::new("SC").unwrap(),
    )
    .unwrap_err();
    let invalid_environment = LogSettings::from_env(
        &snapshot(&[("sc_log_level", "Info")]),
        &EnvPrefix::new("SC").unwrap(),
    )
    .unwrap_err();
    let unknown_key = LogSettings::from_env(
        &snapshot(&[("SC_LOG_UNKNOWN", "x")]),
        &EnvPrefix::new("SC").unwrap(),
    )
    .unwrap_err();
    let prefix_collision =
        LogSettings::from_application_env(&snapshot(&[]), &EnvPrefix::new("SC").unwrap())
            .unwrap_err();
    let resolution = LogSettings::resolve(LogSettingsInputs {
        file: None,
        shared_env: LogSettings::default(),
        application_env: None,
        default_root: PathBuf::new(),
    })
    .unwrap_err();

    let errors = [
        invalid_value,
        invalid_environment,
        unknown_key,
        prefix_collision,
        resolution,
    ];
    let mut all_steps = Vec::new();
    for error in errors {
        let diagnostic = error.context().diagnostic();
        assert_eq!(
            diagnostic.docs.as_deref(),
            Some("docs/logging/d-1-log-settings.md")
        );
        let Remediation::Recoverable { steps } = &diagnostic.remediation else {
            panic!("settings errors must be recoverable");
        };
        assert!(!steps.steps().is_empty(), "{}", error.code());
        all_steps.extend(steps.steps().iter().cloned());
    }

    let all_steps = all_steps.join(" ");
    assert!(all_steps.contains("SC_LOG_ROTATION_MAX_BYTES"));
    assert!(all_steps.contains("SC_LOG_MAINTENANCE_MAX_WORK_PER_PASS"));
    assert!(all_steps.contains("${prefix}_LOG_"));
    assert!(all_steps.contains("lowercase true or false"));
    assert!(all_steps.contains("docs/logging/d-1-log-settings.md"));
}

#[test]
fn empty_default_root_reports_resolution_failure() {
    let error = LogSettings::resolve(LogSettingsInputs {
        file: None,
        shared_env: LogSettings::default(),
        application_env: None,
        default_root: PathBuf::new(),
    })
    .expect_err("empty effective default root cannot be resolved");

    assert!(matches!(error, LogSettingsError::Resolution { .. }));
    assert_eq!(error.code().as_str(), "SC_LOG_SETTINGS_RESOLUTION");
}

fn assert_application_inventory_values(resolved: &ResolvedLogSettings) {
    let policy = resolved.retained_log_policy;
    let actual = [
        ("level", format!("{:?}", resolved.level), "Error".to_owned()),
        (
            "log root (the JSON exception)",
            resolved.log_root.as_ref().display().to_string(),
            "/json".to_owned(),
        ),
        (
            "file sink",
            resolved.enable_file_sink.to_string(),
            "true".to_owned(),
        ),
        (
            "console sink",
            resolved.enable_console_sink.to_string(),
            "false".to_owned(),
        ),
        (
            "rotation bytes",
            policy.rotation_max_bytes.as_u64().to_string(),
            "48".to_owned(),
        ),
        (
            "rotation files",
            policy.rotation_max_files.as_usize().to_string(),
            "4".to_owned(),
        ),
        (
            "retention age",
            policy
                .retention_max_age
                .as_duration()
                .as_millis()
                .to_string(),
            "3000".to_owned(),
        ),
        (
            "maintenance cadence",
            policy
                .maintenance_cadence
                .as_duration()
                .as_millis()
                .to_string(),
            "300".to_owned(),
        ),
        (
            "writer shutdown timeout",
            policy
                .writer_shutdown_timeout
                .as_duration()
                .as_millis()
                .to_string(),
            "300".to_owned(),
        ),
        (
            "maintenance maximum work",
            policy
                .maintenance_max_work_per_pass
                .expect("application policy value")
                .to_string(),
            "3".to_owned(),
        ),
    ];
    for (field, actual, expected) in actual {
        assert_eq!(actual, expected, "{field} must use its documented layer");
    }
}

struct InventoryExpectation {
    level: LevelFilter,
    log_root: &'static str,
    enable_file_sink: bool,
    enable_console_sink: bool,
    rotation_max_bytes: u64,
    rotation_max_files: usize,
    retention_max_age_ms: u128,
    maintenance_cadence_ms: u128,
    writer_shutdown_timeout_ms: u128,
    maintenance_max_work_per_pass: Option<usize>,
}

struct LayerCase {
    name: &'static str,
    file: Option<&'static str>,
    shared_env: &'static [(&'static str, &'static str)],
    application_env: &'static [(&'static str, &'static str)],
    expectation: InventoryExpectation,
}

fn assert_resolved_inventory_values(
    resolved: ResolvedLogSettings,
    expectation: &InventoryExpectation,
) {
    let config = resolved.into_logger_config(ServiceName::new("settings-test").unwrap());
    let policy = config.retained_log_policy;

    assert_eq!(config.level, expectation.level);
    assert_eq!(config.log_root, PathBuf::from(expectation.log_root));
    assert_eq!(config.enable_file_sink, expectation.enable_file_sink);
    assert_eq!(config.enable_console_sink, expectation.enable_console_sink);
    assert_eq!(
        policy.rotation_max_bytes.as_u64(),
        expectation.rotation_max_bytes
    );
    assert_eq!(
        policy.rotation_max_files.as_usize(),
        expectation.rotation_max_files
    );
    assert_eq!(
        policy.retention_max_age.as_duration().as_millis(),
        expectation.retention_max_age_ms
    );
    assert_eq!(
        policy.maintenance_cadence.as_duration().as_millis(),
        expectation.maintenance_cadence_ms
    );
    assert_eq!(
        policy.writer_shutdown_timeout.as_duration().as_millis(),
        expectation.writer_shutdown_timeout_ms
    );
    assert_eq!(
        policy.maintenance_max_work_per_pass,
        expectation.maintenance_max_work_per_pass
    );
}

fn default_inventory_layer() -> LayerCase {
    LayerCase {
        name: "defaults",
        file: None,
        shared_env: &[],
        application_env: &[],
        expectation: InventoryExpectation {
            level: LevelFilter::Info,
            log_root: "/default",
            enable_file_sink: true,
            enable_console_sink: false,
            rotation_max_bytes: 64 * 1024 * 1024,
            rotation_max_files: 10,
            retention_max_age_ms: 7 * 24 * 60 * 60 * 1000,
            maintenance_cadence_ms: 60 * 1000,
            writer_shutdown_timeout_ms: 5 * 1000,
            maintenance_max_work_per_pass: None,
        },
    }
}

fn json_inventory_layer() -> LayerCase {
    LayerCase {
        name: "JSON",
        file: Some(
            r#"{"level":"Warn","logRoot":"/json","enableFileSink":false,"enableConsoleSink":true,"retainedLogPolicy":{"rotation_max_bytes":12,"rotation_max_files":2,"retention_max_age":1000,"maintenance_cadence":100,"writer_shutdown_timeout":100,"maintenance_max_work_per_pass":1}}"#,
        ),
        shared_env: &[],
        application_env: &[],
        expectation: InventoryExpectation {
            level: LevelFilter::Warn,
            log_root: "/json",
            enable_file_sink: false,
            enable_console_sink: true,
            rotation_max_bytes: 12,
            rotation_max_files: 2,
            retention_max_age_ms: 1000,
            maintenance_cadence_ms: 100,
            writer_shutdown_timeout_ms: 100,
            maintenance_max_work_per_pass: Some(1),
        },
    }
}

fn shared_environment_inventory_layer() -> LayerCase {
    LayerCase {
        name: "shared environment",
        file: None,
        shared_env: &[
            ("SC_LOG_LEVEL", "Debug"),
            ("SC_LOG_ROOT", "/shared"),
            ("SC_LOG_FILE", "false"),
            ("SC_LOG_CONSOLE", "true"),
            ("SC_LOG_ROTATION_MAX_BYTES", "24"),
            ("SC_LOG_ROTATION_MAX_FILES", "3"),
            ("SC_LOG_RETENTION_MAX_AGE_MS", "2000"),
            ("SC_LOG_MAINTENANCE_CADENCE_MS", "200"),
            ("SC_LOG_WRITER_SHUTDOWN_TIMEOUT_MS", "200"),
            ("SC_LOG_MAINTENANCE_MAX_WORK_PER_PASS", "2"),
        ],
        application_env: &[],
        expectation: InventoryExpectation {
            level: LevelFilter::Debug,
            log_root: "/shared",
            enable_file_sink: false,
            enable_console_sink: true,
            rotation_max_bytes: 24,
            rotation_max_files: 3,
            retention_max_age_ms: 2000,
            maintenance_cadence_ms: 200,
            writer_shutdown_timeout_ms: 200,
            maintenance_max_work_per_pass: Some(2),
        },
    }
}

fn application_environment_inventory_layer() -> LayerCase {
    LayerCase {
        name: "application environment",
        file: None,
        shared_env: &[],
        application_env: &[
            ("APP_LOG_LEVEL", "Error"),
            ("APP_LOG_ROOT", "/application"),
            ("APP_LOG_FILE", "false"),
            ("APP_LOG_CONSOLE", "true"),
            ("APP_LOG_ROTATION_MAX_BYTES", "48"),
            ("APP_LOG_ROTATION_MAX_FILES", "4"),
            ("APP_LOG_RETENTION_MAX_AGE_MS", "3000"),
            ("APP_LOG_MAINTENANCE_CADENCE_MS", "300"),
            ("APP_LOG_WRITER_SHUTDOWN_TIMEOUT_MS", "300"),
            ("APP_LOG_MAINTENANCE_MAX_WORK_PER_PASS", "3"),
        ],
        expectation: InventoryExpectation {
            level: LevelFilter::Error,
            log_root: "/application",
            enable_file_sink: false,
            enable_console_sink: true,
            rotation_max_bytes: 48,
            rotation_max_files: 4,
            retention_max_age_ms: 3000,
            maintenance_cadence_ms: 300,
            writer_shutdown_timeout_ms: 300,
            maintenance_max_work_per_pass: Some(3),
        },
    }
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
        &EnvPrefix::new("SC").expect("valid prefix"),
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
        &EnvPrefix::new("SC").unwrap(),
    )
    .unwrap();
    let application = LogSettings::from_env(
        &snapshot(&[("APP_LOG_LEVEL", "Error"), ("APP_LOG_ROOT", "/app")]),
        &EnvPrefix::new("APP").unwrap(),
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
fn every_inventory_row_obeys_all_four_resolution_layers() {
    let file: LogSettings = serde_json::from_str(
        r#"{"level":"Warn","logRoot":"/json","enableFileSink":true,"enableConsoleSink":false,"retainedLogPolicy":{"rotation_max_bytes":12,"rotation_max_files":2,"retention_max_age":1000,"maintenance_cadence":100,"writer_shutdown_timeout":100,"maintenance_max_work_per_pass":1}}"#,
    )
    .expect("JSON inventory row values");
    let shared = LogSettings::from_env(
        &snapshot(&[
            ("SC_LOG_LEVEL", "Debug"),
            ("SC_LOG_ROOT", "/shared"),
            ("SC_LOG_FILE", "false"),
            ("SC_LOG_CONSOLE", "true"),
            ("SC_LOG_ROTATION_MAX_BYTES", "24"),
            ("SC_LOG_ROTATION_MAX_FILES", "3"),
            ("SC_LOG_RETENTION_MAX_AGE_MS", "2000"),
            ("SC_LOG_MAINTENANCE_CADENCE_MS", "200"),
            ("SC_LOG_WRITER_SHUTDOWN_TIMEOUT_MS", "200"),
            ("SC_LOG_MAINTENANCE_MAX_WORK_PER_PASS", "2"),
        ]),
        &EnvPrefix::new("SC").expect("valid prefix"),
    )
    .expect("shared environment inventory rows");
    let application = LogSettings::from_env(
        &snapshot(&[
            ("APP_LOG_LEVEL", "Error"),
            ("APP_LOG_ROOT", "/application"),
            ("APP_LOG_FILE", "true"),
            ("APP_LOG_CONSOLE", "false"),
            ("APP_LOG_ROTATION_MAX_BYTES", "48"),
            ("APP_LOG_ROTATION_MAX_FILES", "4"),
            ("APP_LOG_RETENTION_MAX_AGE_MS", "3000"),
            ("APP_LOG_MAINTENANCE_CADENCE_MS", "300"),
            ("APP_LOG_WRITER_SHUTDOWN_TIMEOUT_MS", "300"),
            ("APP_LOG_MAINTENANCE_MAX_WORK_PER_PASS", "3"),
        ]),
        &EnvPrefix::new("APP").expect("valid prefix"),
    )
    .expect("application environment inventory rows");

    let resolved = LogSettings::resolve(LogSettingsInputs {
        file: Some(file),
        shared_env: shared,
        application_env: Some(application),
        default_root: PathBuf::from("/default"),
    })
    .expect("all layers resolve");
    assert_application_inventory_values(&resolved);
}

#[test]
fn every_inventory_row_resolves_from_each_layer_in_isolation() {
    let cases = [
        default_inventory_layer(),
        json_inventory_layer(),
        shared_environment_inventory_layer(),
        application_environment_inventory_layer(),
    ];

    for case in cases {
        let file = case
            .file
            .map(|value| serde_json::from_str(value).expect("valid JSON settings"));
        let shared_env = LogSettings::from_env(
            &snapshot(case.shared_env),
            &EnvPrefix::new("SC").expect("valid shared prefix"),
        )
        .expect("valid shared settings");
        let application_env = (!case.application_env.is_empty()).then(|| {
            LogSettings::from_env(
                &snapshot(case.application_env),
                &EnvPrefix::new("APP").expect("valid application prefix"),
            )
            .expect("valid application settings")
        });

        let resolved = LogSettings::resolve(LogSettingsInputs {
            file,
            shared_env,
            application_env,
            default_root: PathBuf::from("/default"),
        })
        .unwrap_or_else(|error| panic!("{} must resolve: {error}", case.name));
        assert_resolved_inventory_values(resolved, &case.expectation);
    }
}

#[test]
fn a_higher_environment_policy_replaces_json_policy_atomically() {
    let file: LogSettings = serde_json::from_str(
        r#"{"retainedLogPolicy":{"rotation_max_bytes":12,"rotation_max_files":2,"retention_max_age":1000,"maintenance_cadence":100,"writer_shutdown_timeout":100,"maintenance_max_work_per_pass":1}}"#,
    )
    .expect("JSON policy");
    let shared = LogSettings::from_env(
        &snapshot(&[("SC_LOG_ROTATION_MAX_FILES", "9")]),
        &EnvPrefix::new("SC").expect("valid prefix"),
    )
    .expect("shared policy override");
    let resolved = LogSettings::resolve(LogSettingsInputs {
        file: Some(file),
        shared_env: shared,
        application_env: None,
        default_root: PathBuf::from("/default"),
    })
    .expect("policy resolution");

    assert_eq!(
        resolved.retained_log_policy.rotation_max_files.as_usize(),
        9
    );
    assert_eq!(
        resolved.retained_log_policy.rotation_max_bytes,
        RetainedLogPolicy::default().rotation_max_bytes,
        "the higher policy replaces, rather than field-merges with, JSON",
    );
}

#[test]
fn settings_reject_invalid_values_duplicates_and_unknown_json_fields() {
    let invalid_rows = [
        ("SC_LOG_FILE", "not-a-bool"),
        ("SC_LOG_LEVEL", "not-a-level"),
        ("SC_LOG_MAINTENANCE_CADENCE_MS", "0"),
    ];
    for (key, value) in invalid_rows {
        let error = LogSettings::from_env(
            &snapshot(&[(key, value)]),
            &EnvPrefix::new("SC").expect("valid prefix"),
        )
        .expect_err("invalid inventory value must fail");
        assert_eq!(error.code(), error_codes::LOG_INVALID_VALUE, "{key}");
    }

    let duplicate = LogSettings::from_env(
        &snapshot(&[("SC_LOG_LEVEL", "Info"), ("sc_log_level", "Debug")]),
        &EnvPrefix::new("SC").expect("valid prefix"),
    )
    .expect_err("case-folded duplicate must fail");
    assert_eq!(duplicate.code(), error_codes::LOG_INVALID_ENVIRONMENT);

    assert!(serde_json::from_str::<LogSettings>(r#"{"unknownSetting":true}"#).is_err());
}

#[cfg(unix)]
#[test]
fn rejects_non_utf8_value_in_selected_namespace() {
    let snapshot = EnvSnapshot::from_pairs([(
        OsString::from("SC_LOG_LEVEL"),
        OsString::from_vec(b"\xFF".to_vec()),
    )]);
    let error = LogSettings::from_env(&snapshot, &EnvPrefix::new("SC").unwrap()).unwrap_err();
    assert_eq!(error.code(), error_codes::LOG_INVALID_ENVIRONMENT);
}

#[test]
fn settings_conversion_preserves_every_non_inventory_default() {
    let service = ServiceName::new("settings-test").expect("valid service");
    let root = PathBuf::from("/default");
    let config = LogSettings::resolve(LogSettingsInputs {
        file: None,
        shared_env: LogSettings::default(),
        application_env: None,
        default_root: root.clone(),
    })
    .expect("defaults resolve")
    .into_logger_config(service.clone());
    let defaults = sc_observability::LoggerConfig::default_for(service, root);

    assert_eq!(config.service_name, defaults.service_name);
    assert_eq!(config.log_root, defaults.log_root);
    assert_eq!(config.queue_capacity, defaults.queue_capacity);
    assert_eq!(
        config.redaction.denylist_keys,
        defaults.redaction.denylist_keys
    );
    assert_eq!(
        config.redaction.redact_bearer_tokens,
        defaults.redaction.redact_bearer_tokens
    );
    assert!(config.redaction.custom_redactors.is_empty());
    assert!(defaults.redaction.custom_redactors.is_empty());
    assert!(matches!(
        config.process_identity,
        sc_observability_types::ProcessIdentityPolicy::Auto
    ));
    assert!(matches!(
        defaults.process_identity,
        sc_observability_types::ProcessIdentityPolicy::Auto
    ));
    assert_eq!(config.enable_file_sink, defaults.enable_file_sink);
    assert_eq!(config.enable_console_sink, defaults.enable_console_sink);
    assert_eq!(config.retained_log_policy, defaults.retained_log_policy);
}

#[test]
fn permits_empty_root_until_effective_resolution_and_rejects_unknown_case_and_prefix_collision() {
    let empty_root = LogSettings::from_env(
        &snapshot(&[("SC_LOG_ROOT", "")]),
        &EnvPrefix::new("SC").unwrap(),
    )
    .expect("an empty root is deferred to the effective-root validator");
    assert_eq!(empty_root.log_root, Some(PathBuf::new()));
    let unknown = LogSettings::from_env(
        &snapshot(&[("SC_LOG_UNKNOWN", "x")]),
        &EnvPrefix::new("SC").unwrap(),
    )
    .unwrap_err();
    assert_eq!(unknown.code(), error_codes::LOG_UNKNOWN_KEY);
    let empty_unknown = LogSettings::from_env(
        &snapshot(&[("SC_LOG_UNKNOWN", "")]),
        &EnvPrefix::new("SC").unwrap(),
    )
    .unwrap_err();
    assert_eq!(empty_unknown.code(), error_codes::LOG_UNKNOWN_KEY);
    let case = LogSettings::from_env(
        &snapshot(&[("sc_log_level", "Info")]),
        &EnvPrefix::new("SC").unwrap(),
    )
    .unwrap_err();
    assert_eq!(case.code(), error_codes::LOG_INVALID_ENVIRONMENT);
    let collision =
        LogSettings::from_application_env(&snapshot(&[]), &EnvPrefix::new("SC").unwrap())
            .unwrap_err();
    assert!(matches!(
        collision,
        LogSettingsError::PrefixCollision { .. }
    ));
    assert_eq!(collision.code(), error_codes::LOG_PREFIX_COLLISION);

    for error in [&unknown, &empty_unknown, &case, &collision] {
        assert!(
            error_codes::ALL.contains(&error.code()),
            "emitted code {} must be registered",
            error.code(),
        );
    }
}

#[cfg(unix)]
#[test]
fn rejects_non_utf8_key_in_selected_namespace() {
    let snapshot = EnvSnapshot::from_pairs([(
        OsString::from_vec(b"SC_LOG_\xFF".to_vec()),
        OsString::from("ignored"),
    )]);

    let error = LogSettings::from_env(&snapshot, &EnvPrefix::new("SC").unwrap()).unwrap_err();
    assert_eq!(error.code(), error_codes::LOG_INVALID_ENVIRONMENT);
}

#[cfg(unix)]
#[test]
fn ignores_non_utf8_key_outside_selected_namespace() {
    let snapshot = EnvSnapshot::from_pairs([
        (
            OsString::from_vec(b"UNRELATED_\xFF".to_vec()),
            OsString::from("ignored"),
        ),
        (OsString::from("SC_LOG_LEVEL"), OsString::from("Info")),
    ]);

    let settings = LogSettings::from_env(&snapshot, &EnvPrefix::new("SC").unwrap())
        .expect("unrelated non-UTF-8 key must be ignored");
    assert_eq!(settings.level, Some(LevelFilter::Info));
}

#[cfg(windows)]
#[test]
fn rejects_non_utf8_key_in_selected_namespace() {
    let snapshot = EnvSnapshot::from_pairs([(
        OsString::from_wide(&[
            0x0053, 0x0043, 0x005F, 0x004C, 0x004F, 0x0047, 0x005F, 0xD800,
        ]),
        OsString::from("ignored"),
    )]);

    let error = LogSettings::from_env(&snapshot, &EnvPrefix::new("SC").unwrap()).unwrap_err();
    assert_eq!(error.code(), error_codes::LOG_INVALID_ENVIRONMENT);
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
    assert_eq!(error.code(), error_codes::LOG_RESOLUTION);
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

#[test]
fn shadowed_empty_root_is_ignored_before_effective_root_validation() {
    let shared_env = LogSettings::from_env(
        &snapshot(&[("SC_LOG_ROOT", "")]),
        &EnvPrefix::new("SC").unwrap(),
    )
    .expect("an empty shadowed root parses for precedence resolution");
    let file: LogSettings = serde_json::from_str(r#"{"logRoot":"/json"}"#).unwrap();

    let resolved = LogSettings::resolve(LogSettingsInputs {
        file: Some(file),
        shared_env,
        application_env: None,
        default_root: PathBuf::from("/default"),
    })
    .expect("the non-empty effective root is the only validated root");

    assert_eq!(resolved.log_root.as_ref(), std::path::Path::new("/json"));
}
