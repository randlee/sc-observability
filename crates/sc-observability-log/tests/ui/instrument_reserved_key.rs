// valid in tracing-attributes 0.1.31, deliberately rejected: reserved `sc_observability_log.` keys
use sc_observability_log::instrument;

#[instrument(fields(sc_observability_log.x = 1))]
fn reserved_key() {}

fn main() {
    reserved_key();
}
