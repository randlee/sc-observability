// valid in tracing 0.1.44, deliberately rejected: reserved string key (also inside braces)
use sc_observability_log::info;

fn main() {
    info!({ "sc_observability_log.x" = 1 }, "message");
}
