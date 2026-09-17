// not valid in tracing 0.1.44 either: the value implements neither Serialize nor Debug
use sc_observability_log::info;

struct Neither;

fn main() {
    let neither = Neither;
    info!(field = neither, "message");
}
