// valid in tracing-attributes 0.1.31, deliberately rejected: deferred fields (`field::Empty`)
use sc_observability_log::instrument;

#[instrument(fields(x))]
fn deferred_ident() {}

#[instrument(fields(a.b))]
fn deferred_dotted() {}

fn main() {
    deferred_ident();
    deferred_dotted();
}
