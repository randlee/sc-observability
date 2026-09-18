// R-A4-005: a `LogControl` (and so its operations) exists only through `LogGuard::control`.
fn forge_control() -> sc_observability_log::LogControl {
    sc_observability_log::LogControl { _private: () }
}

fn main() {}
