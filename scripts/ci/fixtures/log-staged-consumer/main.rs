use sc_observability_log::{BridgeOptions, LoggerConfig, ServiceName};
use std::{path::PathBuf, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(std::env::args_os().nth(1).ok_or("missing log directory")?);
    let config = LoggerConfig::default_for(ServiceName::new("b2-staged-consumer")?, directory);
    let guard = sc_observability_log::init(config, BridgeOptions::default())?;
    let path = guard
        .active_log_path()
        .ok_or("missing active log path")?
        .to_path_buf();
    sc_observability_log::info!(
        qualification = "B.2",
        count = 7_u64,
        "immutable staged macro"
    );
    guard.flush(Duration::from_secs(5))?;
    guard.shutdown(Duration::from_secs(5))?;
    let lines = std::fs::read_to_string(path)?;
    let records = lines
        .lines()
        .map(serde_json::from_str::<serde_json::Value>)
        .collect::<Result<Vec<_>, _>>()?;
    let record = records
        .iter()
        .find(|row| row["message"] == "immutable staged macro")
        .ok_or("enabled macro did not persist")?;
    assert_eq!(record["fields"]["qualification"], "B.2");
    assert_eq!(record["fields"]["count"], 7);
    println!("B2_CONSUMER_OK enabled_macro explicit_flush explicit_shutdown jsonl_content");
    Ok(())
}
