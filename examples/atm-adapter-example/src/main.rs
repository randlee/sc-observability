//! Unpublished proving artifact for the ATM adapter pattern.
//!
//! This crate is intentionally minimal. It exists to prove that ATM-shaped
//! payloads can be modeled outside the shared repo while depending only on the
//! standalone observability crates.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AtmHookObservation {
    event_type: String,
    agent_id: String,
    team: String,
}

fn main() {
    let _sample = AtmHookObservation {
        event_type: "tool_use".to_string(),
        agent_id: "agent-123".to_string(),
        team: "atm-dev".to_string(),
    };
}
