const COMMANDS: &[&str] = &[
    "sc_observability_try_log",
    "sc_observability_query",
    "sc_observability_health",
    "sc_observability_flush",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
