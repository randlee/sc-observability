// not valid in tracing-attributes 0.1.31 either (`expected ident`): covers `""` and reserved strings
use sc_observability_log::instrument;

#[instrument(fields("k" = 1))]
fn literal_key() {}

#[instrument(fields("" = 1))]
fn empty_literal_key() {}

#[instrument(fields("sc_observability_log.x" = 1))]
fn reserved_literal_key() {}

fn main() {
    literal_key();
    empty_literal_key();
    reserved_literal_key();
}
