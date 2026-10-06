//! Compile-time contract for the canonical logger typestate API.

#[cfg(feature = "v1")]
#[test]
fn v2_logger_rejects_operations_after_shutdown() {
    trybuild::TestCases::new().compile_fail("tests/ui/v2_logger_stopped_operations.rs");
}
