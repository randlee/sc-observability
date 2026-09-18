// valid in tracing-attributes 0.1.31, deliberately rejected: no span parents
use sc_observability_log::instrument;

#[instrument(parent = None)]
fn with_parent() {}

fn main() {
    with_parent();
}
