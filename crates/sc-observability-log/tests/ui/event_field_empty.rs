// valid in tracing 0.1.44, deliberately rejected: deferred fields
use sc_observability_log::info;

fn main() {
    info!(k = tracing::field::Empty, "message");
}
