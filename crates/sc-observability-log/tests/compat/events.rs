// Shared compatibility fixture: one call per supported tracing 0.1 event form
// (the grammar table in `docs/compatibility.md`). Not a test binary: it is
// `include!`d by `tests/compat_events.rs` twice, once after
// `use tracing::{trace, debug, info, warn, error, event, Level};` (compile-only
// proof of genuine tracing syntax) and once after the same import from
// `sc_observability_log` (executed and asserted), and by
// `sc-observability-log-consumer-check` (single-dependency proof). The includer
// supplies the macro and `Level` imports; rejected forms live in `tests/ui/`.

/// Emits one event per supported grammar form, in grammar-table order.
pub fn emit_compat_events() {
    const TGT: &str = "compat.const_target";
    const NAME: &str = "compat.const_name";
    const KEY: &str = "compat.dynamic";
    const LVL: Level = Level::WARN;

    struct Point {
        x: i64,
    }

    let point = Point { x: 3 };
    let count: i64 = 7;
    let ok = true;
    let label = "beads";
    let path = "a/b";

    // Format message only.
    info!("compat: plain {}", count);
    // `target:` forms.
    info!(target: "compat.literal_target", "compat: target literal");
    info!(target: TGT, "compat: target const");
    info!(target: module_path!(), "compat: target module_path");
    info!(target: concat!("compat", ".concat_target"), "compat: target concat");
    // `name:` forms.
    info!(name: "compat.literal_name", "compat: name literal");
    info!(name: NAME, "compat: name const");
    info!(name: concat!("compat", ".concat_name"), "compat: name concat");
    // `name:` then `target:`.
    info!(name: "compat.both_name", target: "compat.both_target", "compat: name and target");
    // Keys.
    info!(k = count, "compat: key");
    info!(a.b = count, "compat: dotted key");
    info!("literal key" = count, "compat: literal key");
    info!(r#type = label, "compat: raw key");
    info!({ KEY } = count, "compat: dynamic key");
    info!({ KEY } = ?label, "compat: dynamic key debug");
    info!({ KEY } = %path, "compat: dynamic key display");
    // `?` and `%`.
    info!(?label, "compat: debug shorthand");
    info!(k = ?label, "compat: debug value");
    info!(%path, "compat: display shorthand");
    info!(k = %path, "compat: display value");
    // Shorthand.
    info!(count, "compat: shorthand");
    info!(point.x, "compat: dotted shorthand");
    info!(compat_shorthand_only = ok);
    // Brace field sets.
    info!({ k = 1, ?label }, "compat: brace {}", count);
    info!({ compat_brace_only = 1 });
    info!(name: "compat.brace_name", { k = 1 }, "compat: brace after name");
    info!(target: "compat.brace_target", { k = 1 }, "compat: brace after target");
    info!(name: "compat.brace_both_name", target: "compat.brace_both_target", { k = 1 }, "compat: brace after both");
    event!(Level::INFO, { k = 1 }, "compat: event brace");
    // Fields followed by a message.
    info!(k = count, ok, "compat: fields then message {}", label);
    // `event!` forms.
    event!(Level::INFO, "compat: event level");
    event!(target: "compat.event_target", Level::WARN, "compat: event target");
    event!(name: "compat.event_name", Level::INFO, "compat: event name");
    event!(name: "compat.event_both_name", target: "compat.event_both_target", Level::ERROR, "compat: event both");
    event!(LVL, "compat: event const level");
    // Every leveled macro.
    trace!(k = 1, "compat: trace");
    debug!(k = 1, "compat: debug");
    warn!(k = 1, "compat: warn");
    error!(k = 1, "compat: error");
}
