//! Compile-time contracts for the public logger typestate API.

#[test]
fn stopped_logger_operations_fail_and_shutdown_transitions() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/logger_stopped_operations.rs");
    cases.pass("tests/ui/logger_running_to_stopped.rs");
}
