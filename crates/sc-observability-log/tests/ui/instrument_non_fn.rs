// not valid in tracing-attributes 0.1.31 either: `#[instrument]` applies to functions only
use sc_observability_log::instrument;

#[instrument]
struct NotAFunction;

fn main() {
    let _ = NotAFunction;
}
