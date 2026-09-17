// valid in tracing 0.1.44, deliberately rejected: span macros are not exported
fn main() {
    let _span = sc_observability_log::info_span!("span");
}
