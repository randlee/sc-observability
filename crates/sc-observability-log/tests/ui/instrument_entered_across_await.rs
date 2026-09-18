// `Entered` is `!Send`: holding it across `.await` makes the future `!Send` (Deliverable 7).
use sc_observability_log::__private::{CallLevels, CallSpan, Callsite, Level, Map};

static CALLSITE: Callsite = Callsite::new("ui", Some("entered_across_await"));

async fn yield_point() {}

fn require_send<F: std::future::Future + Send>(future: F) -> F {
    future
}

fn main() {
    let levels = CallLevels {
        level: Level::Info,
        ok: Level::Info,
        error: Level::Error,
    };
    let _ = require_send(async move {
        let span = CallSpan::new(&CALLSITE, levels, Map::new);
        let _entered = span.enter();
        yield_point().await;
    });
}
