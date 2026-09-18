// valid in tracing 0.1.44, deliberately rejected: empty field key
use sc_observability_log::info;

fn main() {
    info!("" = 1, "message");
}
