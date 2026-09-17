// not valid in tracing 0.1.44 either: `name:` must precede `target:`
use sc_observability_log::info;

fn main() {
    info!(target: "t", name: "n", "message");
}
