// R-A4-005: `LogGuard` is the sole lifecycle owner and cannot be cloned.
fn requires_clone<T: Clone>() {}

fn main() {
    requires_clone::<sc_observability_log::LogGuard>();
}
