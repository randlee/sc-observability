// valid in tracing-attributes 0.1.31, deliberately rejected: deferred fields (`field::Empty`)
use sc_observability_log::instrument;

#[instrument(fields(x = tracing::field::Empty))]
fn field_empty() {}

fn main() {
    field_empty();
}
