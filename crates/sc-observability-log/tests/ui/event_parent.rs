// valid in tracing 0.1.44, deliberately rejected: no span parents
use sc_observability_log::info;

fn main() {
    let span = ();
    info!(parent: span, "message");
}
