// valid in tracing-attributes 0.1.31, deliberately rejected: no span links
use sc_observability_log::instrument;

#[instrument(follows_from = [1_u64])]
fn with_follows_from() {}

fn main() {
    with_follows_from();
}
