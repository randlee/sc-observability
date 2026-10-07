//! Command-line parsing for the `sc-otel` executable.

#[cfg(test)]
use clap::CommandFactory;
use clap::{Args, Parser, Subcommand, ValueEnum};
#[cfg(test)]
use sc_observability_types::otlp::submission::ConfigOverrides;
#[cfg(test)]
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};

#[derive(Debug, Parser)]
#[command(
    name = "sc-otel",
    version,
    about = "Submit and inspect durable telemetry",
    long_about = "Submit logs, spans, metrics, and profiles through a durable local queue to an OTLP collector. Delivery is at least once: retained records can be resent after interrupted or uncertain delivery.",
    after_long_help = include_str!("manual.txt")
)]
pub(crate) struct Cli {
    /// Read telemetry YAML from this explicit path. No configuration file is auto-discovered. Relative store.path values in YAML are resolved against this file's directory. validate does not load configuration.
    #[arg(long, global = true)]
    pub(crate) config: Option<PathBuf>,
    /// Use this `SQLite` queue path instead of store.path in YAML. Required for emit, flush, and status unless supplied in the file; no environment fallback. Relative CLI paths use the current directory.
    #[arg(long, global = true)]
    pub(crate) store: Option<PathBuf>,
    /// Override the OTLP HTTP base URL. Precedence: this flag, otlp.endpoint in YAML, `OTEL_EXPORTER_OTLP_ENDPOINT`, then <http://localhost:4318>.
    #[arg(long, global = true)]
    pub(crate) endpoint: Option<String>,
    /// Choose machine-readable JSON (default) or one compact text result line. Parser errors and help use Clap's normal diagnostics regardless of this setting.
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Json)]
    pub(crate) output: OutputFormat,
    #[command(subcommand)]
    pub(crate) command: Command,
    #[cfg(test)]
    #[arg(skip)]
    pub(crate) unit_client_paths: Option<crate::client::UnitClientPaths>,
    #[cfg(test)]
    #[arg(skip)]
    pub(crate) unit_config_overrides: Option<ConfigOverrides>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum OutputFormat {
    Json,
    Text,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Validate and durably admit telemetry, then attempt delivery.
    #[command(
        long_about = "Parse and validate the input, open the configured durable store, and admit the submission before delivery. Normally flushes this submission and shuts down the client; --no-flush returns after admission. A receipt identifies admitted records even if delivery later fails.",
        after_long_help = "Examples:\n  sc-otel --store queue.sqlite emit --log '{\"body\":\"started\"}' --no-flush\n  sc-otel --config telemetry.yaml emit --stdin < event.json\n  sc-otel --store queue.sqlite emit --log @log.json --record-key job-42\n\nUse --stdin OR fragment flags; at least one input source is required. With stdin, a record_key belongs inside the input JSON. Keep the store after exit 6 so flush can retry delivery."
    )]
    Emit(EmitArgs),
    /// Validate input locally without opening a store or contacting a collector.
    #[command(
        long_about = "Parse SubmissionInput and construct a normalized SubmissionEnvelope, including generated identifiers and timestamps. The JSON result includes the normalized envelope. This checks input shape and values, not collector availability or successful delivery; global configuration flags are not loaded.",
        after_long_help = "Examples:\n  sc-otel validate --log '{\"body\":\"started\"}'\n  sc-otel validate --stdin < event.json\n  sc-otel validate --log @log.json --span @span.json\n\nFragment files are UTF-8 JSON, one signal value per file. Repeating --log, --span, or --metric appends values; --profile accepts one profiles object. Do not supply generated envelope fields such as submission_id in SubmissionInput."
    )]
    Validate(InputArgs),
    /// Attempt delivery of pending records in the configured durable store.
    #[command(
        after_long_help = "Examples:\n  sc-otel --config telemetry.yaml flush --timeout 10\n  sc-otel --store queue.sqlite --endpoint http://localhost:4318 flush\n\nThis command flushes the store, not one selected submission. Exit 6 means records remain pending; retain the queue and retry when the collector is available. Exit 7 means a terminal delivery failure. A flush deadline is not a wall-clock bound for opening and shutting down the process."
    )]
    Flush(FlushArgs),
    /// Inspect the queue summary or selected submissions/record keys.
    #[command(
        after_long_help = "Examples:\n  sc-otel --store queue.sqlite status\n  sc-otel --store queue.sqlite status --record-key job-42\n  sc-otel --store queue.sqlite status --submission 018f8f5e-5c4c-7abc-8def-0123456789ab\n\nRepeat one selector kind to query multiple records; do not mix --submission with --record-key. Status opens the durable client only to inspect queued state; it does not attempt delivery during shutdown."
    )]
    Status(StatusArgs),
}

#[derive(Debug, Args)]
#[group(id = "source", required = true, multiple = true)]
pub(crate) struct InputArgs {
    /// Read one complete `SubmissionInput` JSON document from stdin. Cannot combine with fragment flags. Requires version: 1; use `record_key` inside the document for deduplication.
    #[arg(long, group = "source", conflicts_with_all = ["log", "span", "metric", "profile"])]
    pub(crate) stdin: bool,
    /// Append one log point as inline JSON or @FILE. Repeat to submit multiple logs; can combine with other fragment types.
    #[arg(long, group = "source", value_name = "JSON|@FILE")]
    pub(crate) log: Vec<String>,
    /// Append one span point as inline JSON or @FILE. Repeat to submit multiple spans; can combine with other fragment types.
    #[arg(long, group = "source", value_name = "JSON|@FILE")]
    pub(crate) span: Vec<String>,
    /// Append one metric stream as inline JSON or @FILE. Repeat for multiple streams; can combine with other fragment types.
    #[arg(long, group = "source", value_name = "JSON|@FILE")]
    pub(crate) metric: Vec<String>,
    /// Supply the profiles object as inline JSON or @FILE. May be combined with log, span, and metric fragments.
    #[arg(long, group = "source", value_name = "JSON|@FILE")]
    pub(crate) profile: Option<String>,
    #[cfg(test)]
    #[arg(skip)]
    pub(crate) stdin_input: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct EmitArgs {
    #[command(flatten)]
    pub(crate) input: InputArgs,
    /// Deduplicate a fragment-mode submission with this caller-owned key. With --stdin, put `record_key` in the JSON document instead.
    #[arg(long, conflicts_with = "stdin")]
    pub(crate) record_key: Option<String>,
    /// Return after durable admission without initiating a flush. Exit 0 confirms admission only, not delivery; retain the store and later run flush.
    #[arg(long)]
    pub(crate) no_flush: bool,
}

#[derive(Debug, Args)]
pub(crate) struct FlushArgs {
    /// Bound this flush in whole, unsigned seconds (including 0). Defaults to the resolved flush deadline, currently 30 seconds. Shutdown uses its own configured deadline.
    #[arg(long, value_name = "SECONDS", value_parser = parse_seconds)]
    pub(crate) timeout: Option<Duration>,
}

#[derive(Debug, Args)]
pub(crate) struct StatusArgs {
    /// Select a submission by UUID; repeat for multiple submissions. Mutually exclusive with --record-key. With neither selector, show a queue summary.
    #[arg(long, conflicts_with = "record_key")]
    pub(crate) submission: Vec<String>,
    /// Select records by caller-owned key; repeat for multiple keys. Cannot combine with --submission.
    #[arg(long)]
    pub(crate) record_key: Vec<String>,
}

fn parse_seconds(value: &str) -> Result<Duration, String> {
    value
        .parse::<u64>()
        .map(Duration::from_secs)
        .map_err(|_| "timeout must be an unsigned number of seconds".into())
}

/// Projects the command produced by clap into a normalized, serializable input
/// contract.  This deliberately reflects clap's built [`Command`] instead of
/// scraping help text or maintaining a second parser specification.
#[cfg(test)]
pub(crate) fn command_contract() -> Value {
    command_contract_for(&Cli::command())
}

#[cfg(test)]
fn command_contract_for(command: &clap::Command) -> Value {
    let mut arguments = command
        .get_arguments()
        .map(|argument| {
            let action = argument.get_action();
            let num_args = effective_num_args(argument);
            let mut conflicts = command
                .get_arg_conflicts_with(argument)
                .into_iter()
                .map(|conflict| conflict.get_id().as_str().to_owned())
                .collect::<Vec<_>>();
            conflicts.sort();
            let mut flags = argument
                .get_long_and_visible_aliases()
                .unwrap_or_default()
                .into_iter()
                .map(|name| format!("--{name}"))
                .chain(
                    argument
                        .get_short_and_visible_aliases()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|name| format!("-{name}")),
                )
                .collect::<Vec<_>>();
            flags.sort();
            let mut allowed_values = argument
                .get_possible_values()
                .into_iter()
                .flat_map(|value| {
                    value
                        .get_name_and_aliases()
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            allowed_values.sort();
            allowed_values.dedup();
            let value_names = argument.get_value_names().map(|names| {
                names
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            });
            let requires = required_groups_for(command, argument);
            let mut contract = json!({
                "id": argument.get_id().as_str(),
                "flags": flags,
                "action": format!("{action:?}"),
                "num_args": {
                    "min": num_args.min_values(),
                    "max": num_args.max_values(),
                },
                "global": argument.is_global_set(),
                "value_name": value_names,
                "requires": requires,
                "required": argument.is_required_set(),
                "default_values": argument.get_default_values().iter().map(|value| value.to_string_lossy()).collect::<Vec<_>>(),
                "allowed_values": allowed_values,
                "conflicts_with": conflicts,
            });
            if let Some(constraint) = parser_constraint(command, argument) {
                contract["value_constraint"] = json!(constraint);
            }
            contract
    })
        .collect::<Vec<_>>();
    arguments.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    let mut groups = command
        .get_groups()
        .map(|group| {
            let mut group = group.clone();
            let mut members = group
                .get_args()
                .map(|member| member.as_str().to_owned())
                .collect::<Vec<_>>();
            members.sort();
            json!({
                "id": group.get_id().as_str(),
                "required": group.is_required_set(),
                "multiple": group.is_multiple(),
                "members": members,
            })
        })
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));

    let mut subcommands = command
        .get_subcommands()
        .map(command_contract_for)
        .collect::<Vec<_>>();
    subcommands.sort_by(|left, right| left["name"].as_str().cmp(&right["name"].as_str()));

    json!({
        "name": command.get_name(),
        "aliases": command.get_all_aliases().collect::<Vec<_>>(),
        "arguments": arguments,
        "groups": groups,
        "subcommands": subcommands,
    })
}

#[cfg(test)]
fn effective_num_args(argument: &clap::Arg) -> clap::builder::ValueRange {
    argument.get_num_args().unwrap_or_else(|| {
        if argument.get_action().takes_values() {
            clap::builder::ValueRange::SINGLE
        } else {
            clap::builder::ValueRange::EMPTY
        }
    })
}

/// Clap exposes this CLI's required relationships as required argument groups.
/// Project a member's group identity beside the group definition so a group
/// membership change also identifies the affected argument.
#[cfg(test)]
fn required_groups_for(command: &clap::Command, argument: &clap::Arg) -> Vec<String> {
    let mut groups = command
        .get_groups()
        .filter_map(|group| {
            let group = group.clone();
            (group.is_required_set()
                && group
                    .get_args()
                    .any(|member| member.as_str() == argument.get_id().as_str()))
            .then(|| group.get_id().as_str().to_owned())
        })
        .collect::<Vec<_>>();
    groups.sort();
    groups
}

/// Captures validation supplied by clap's configured parser without creating a
/// parallel parser description. Enumerated parsers are already represented by
/// `allowed_values`; for other value-taking arguments, a stable invalid probe
/// records the parser's own diagnostic only when that parser rejects it.
#[cfg(test)]
fn parser_constraint(command: &clap::Command, argument: &clap::Arg) -> Option<String> {
    const INVALID_VALUE: &str = "__sc_otel_contract_invalid_value__";

    if !argument.get_action().takes_values() || !argument.get_possible_values().is_empty() {
        return None;
    }

    let flag = argument
        .get_long()
        .map(|name| format!("--{name}"))
        .or_else(|| argument.get_short().map(|name| format!("-{name}")))?;
    let probe = command.clone();
    match probe.try_get_matches_from([command.get_name(), flag.as_str(), INVALID_VALUE]) {
        Err(error) if error.kind() == clap::error::ErrorKind::ValueValidation => {
            Some(error.to_string())
        }
        _ => None,
    }
}

#[cfg(test)]
mod contract_tests {
    use super::command_contract;
    use crate::{
        constants::{
            COMMAND_CONTRACT_VERSION, SNAPSHOT_HISTORY_BASELINE,
            UNACCEPTED_INITIAL_COMMAND_SNAPSHOT, UNACCEPTED_INITIAL_RESULT_SNAPSHOT,
        },
        output::result_contract,
    };
    use serde_json::Value;
    use std::{
        collections::{BTreeMap, BTreeSet},
        path::{Path, PathBuf},
        process::Command,
    };

    fn snapshot(path: impl AsRef<Path>) -> Value {
        let contents = std::fs::read_to_string(path).expect("checked-in contract snapshot exists");
        snapshot_json(&contents)
    }

    fn snapshot_bytes(path: impl AsRef<Path>) -> Vec<u8> {
        std::fs::read(path).expect("checked-in contract snapshot exists")
    }

    fn snapshot_json(contents: &str) -> Value {
        serde_json::from_str(contents).expect("contract snapshot is valid JSON")
    }

    fn snapshot_path(kind: &str, version: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../schema/cli/sc-otel")
            .join(kind)
            .join(format!("{version}.json"))
    }

    fn repository_path(path: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path)
    }

    fn member_id(value: &Value) -> Option<&str> {
        value["id"].as_str().or_else(|| value["name"].as_str())
    }

    fn members_by_id(values: &[Value]) -> Option<BTreeMap<&str, &Value>> {
        let members = values
            .iter()
            .map(|value| member_id(value).map(|id| (id, value)))
            .collect::<Option<BTreeMap<_, _>>>()?;
        (members.len() == values.len()).then_some(members)
    }

    fn changed_fields(expected: &Value, actual: &Value, path: &str, changes: &mut Vec<String>) {
        match (expected, actual) {
            (Value::Object(expected), Value::Object(actual)) => {
                for key in expected.keys().chain(actual.keys()) {
                    let next = format!("{path}.{key}");
                    match (expected.get(key), actual.get(key)) {
                        (Some(expected), Some(actual)) => {
                            changed_fields(expected, actual, &next, changes);
                        }
                        _ => changes.push(next),
                    }
                }
            }
            (Value::Array(expected), Value::Array(actual)) => {
                if let (Some(expected), Some(actual)) =
                    (members_by_id(expected), members_by_id(actual))
                {
                    let ids = expected
                        .keys()
                        .chain(actual.keys())
                        .copied()
                        .collect::<BTreeSet<_>>();
                    for id in ids {
                        let next = format!("{path}[{id}]");
                        match (expected.get(id), actual.get(id)) {
                            (Some(expected), Some(actual)) => {
                                changed_fields(expected, actual, &next, changes);
                            }
                            _ => changes.push(next),
                        }
                    }
                } else {
                    for index in 0..expected.len().max(actual.len()) {
                        let next = format!("{path}[{index}]");
                        match (expected.get(index), actual.get(index)) {
                            (Some(expected), Some(actual)) => {
                                changed_fields(expected, actual, &next, changes);
                            }
                            _ => changes.push(next),
                        }
                    }
                }
            }
            _ if expected != actual => changes.push(path.to_owned()),
            _ => {}
        }
    }

    fn assert_contract(
        contract: &str,
        version: &str,
        expected: &Value,
        actual: &Value,
    ) -> Result<(), String> {
        if expected == actual {
            return Ok(());
        }
        let mut changes = Vec::new();
        changed_fields(expected, actual, "$", &mut changes);
        changes.sort();
        changes.dedup();
        Err(format!(
            "{contract} contract version {version} changed fields: {}. If intentional, add a new versioned snapshot; do not overwrite an accepted snapshot.",
            changes.join(", ")
        ))
    }

    fn resolve_history_baseline() -> String {
        let output = Command::new("git")
            .args([
                "rev-parse",
                "--verify",
                &format!("{SNAPSHOT_HISTORY_BASELINE}^{{commit}}"),
            ])
            .output()
            .expect("git must be available for contract snapshot history checks");
        assert!(
            output.status.success(),
            "cannot resolve contract snapshot baseline {SNAPSHOT_HISTORY_BASELINE}: {}",
            String::from_utf8_lossy(&output.stderr),
        );
        let resolved = String::from_utf8(output.stdout)
            .expect("git revision output is UTF-8")
            .trim()
            .to_owned();
        assert_eq!(
            resolved, SNAPSHOT_HISTORY_BASELINE,
            "contract snapshot baseline must be a full, exact commit SHA"
        );
        resolved
    }

    fn is_unaccepted_initial_draft(path: &str) -> bool {
        matches!(
            path,
            UNACCEPTED_INITIAL_COMMAND_SNAPSHOT | UNACCEPTED_INITIAL_RESULT_SNAPSHOT
        )
    }

    fn assert_initial_history(path: &str, expected: &[u8]) {
        let baseline = resolve_history_baseline();
        let output = Command::new("git")
            .args(["show", &format!("{baseline}:{path}")])
            .output()
            .expect("git must be available for contract snapshot history checks");
        if output.status.success() {
            assert_eq!(
                output.stdout, expected,
                "accepted baseline bytes changed at {path}; add a new versioned snapshot instead"
            );
        } else {
            let path_in_baseline = Command::new("git")
                .args(["ls-tree", "--name-only", "-r", &baseline, "--", path])
                .output()
                .expect("git must be available for contract snapshot history checks");
            assert!(
                path_in_baseline.status.success(),
                "cannot inspect contract snapshot baseline {baseline}: {}",
                String::from_utf8_lossy(&path_in_baseline.stderr),
            );
            assert!(
                path_in_baseline.stdout.is_empty() && is_unaccepted_initial_draft(path),
                "cannot read retained contract snapshot {path} from {baseline}: {}",
                String::from_utf8_lossy(&output.stderr),
            );
        }
    }

    fn assert_retained_snapshot_history(kind: &str) {
        let baseline = resolve_history_baseline();
        let snapshot_directory = format!("schema/cli/sc-otel/{kind}");
        let retained_paths = Command::new("git")
            .args([
                "ls-tree",
                "--name-only",
                "-r",
                &baseline,
                "--",
                &snapshot_directory,
            ])
            .output()
            .expect("git must be available for contract snapshot history checks");
        assert!(
            retained_paths.status.success(),
            "cannot inspect retained contract snapshots in {baseline}: {}",
            String::from_utf8_lossy(&retained_paths.stderr),
        );

        for path in String::from_utf8(retained_paths.stdout)
            .expect("git path output is UTF-8")
            .lines()
            .filter(|path| {
                Path::new(path)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
            })
        {
            let expected = snapshot_bytes(repository_path(path));
            let actual = Command::new("git")
                .args(["show", &format!("{baseline}:{path}")])
                .output()
                .expect("git must be available for contract snapshot history checks");
            assert!(
                actual.status.success(),
                "cannot read retained contract snapshot {path} from {baseline}: {}",
                String::from_utf8_lossy(&actual.stderr),
            );
            assert_eq!(
                actual.stdout, expected,
                "accepted baseline bytes changed at {path}; add a new versioned snapshot instead"
            );
        }
    }

    #[test]
    fn current_command_tree_matches_selected_manifest_version() {
        let path = format!("schema/cli/sc-otel/commands/{COMMAND_CONTRACT_VERSION}.json");
        let expected_path = snapshot_path("commands", COMMAND_CONTRACT_VERSION);
        let expected = snapshot(&expected_path);
        assert_retained_snapshot_history("commands");
        assert_initial_history(&path, &snapshot_bytes(&expected_path));
        assert_contract(
            "sc-otel.commands",
            COMMAND_CONTRACT_VERSION,
            &expected,
            &command_contract(),
        )
        .expect("current clap command tree must match its selected snapshot");
    }

    #[test]
    fn current_json_result_matches_selected_identity() {
        let result_version = crate::constants::RESULT_SCHEMA
            .rsplit('/')
            .next()
            .expect("result schema identity has a version");
        let path = format!("schema/cli/sc-otel/results/{result_version}.json");
        let expected_path = snapshot_path("results", result_version);
        let expected = snapshot(&expected_path);
        assert_retained_snapshot_history("results");
        assert_initial_history(&path, &snapshot_bytes(&expected_path));
        assert_contract(
            "sc-otel.result",
            result_version,
            &expected,
            &result_contract(),
        )
        .expect("current JSON result renderer must match its selected snapshot");
    }

    #[test]
    fn changed_command_or_result_is_rejected_with_versioned_remedy() {
        let expected = snapshot(snapshot_path("commands", COMMAND_CONTRACT_VERSION));
        let mut changed_command = command_contract();
        let flush = changed_command["subcommands"]
            .as_array_mut()
            .and_then(|subcommands| {
                subcommands
                    .iter_mut()
                    .find(|subcommand| subcommand["name"] == "flush")
            })
            .expect("the command contract contains the flush subcommand");
        let timeout = flush["arguments"]
            .as_array_mut()
            .and_then(|arguments| {
                arguments
                    .iter_mut()
                    .find(|argument| argument["id"] == "timeout")
            })
            .expect("the flush subcommand contains the timeout argument");
        assert!(
            timeout["value_constraint"].is_string(),
            "timeout parser constraint was not projected: {timeout}"
        );
        assert!(
            timeout["action"].is_string(),
            "argument action was not projected: {timeout}"
        );
        timeout["action"] = serde_json::json!("Append");
        let command_error = assert_contract(
            "sc-otel.commands",
            COMMAND_CONTRACT_VERSION,
            &expected,
            &changed_command,
        )
        .expect_err("changed argument must reject the selected command contract");
        assert!(command_error.contains("sc-otel.commands contract version 1.5.0 changed fields"));
        assert!(
            command_error.contains("$.subcommands[flush].arguments[timeout].action"),
            "{command_error}"
        );
        assert!(command_error.contains("new versioned snapshot"));

        let result_version = crate::constants::RESULT_SCHEMA
            .rsplit('/')
            .next()
            .expect("result schema identity has a version");
        let expected = snapshot(snapshot_path("results", result_version));
        let mut changed_result = result_contract();
        changed_result["fields"]["state"]["enum"] = serde_json::json!(["changed"]);
        let result_error =
            assert_contract("sc-otel.result", result_version, &expected, &changed_result)
                .expect_err("changed response must reject the selected result contract");
        assert!(result_error.contains("sc-otel.result contract version v1 changed fields"));
        assert!(result_error.contains("new versioned snapshot"));
    }
}
