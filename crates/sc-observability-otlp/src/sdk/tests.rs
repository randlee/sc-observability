//! Focused caller-runtime tests for the official SDK adapter.

use super::implementation::CallerRuntime;

#[test]
fn sdk_adapter_requires_an_entered_caller_runtime() {
    assert!(CallerRuntime::try_capture().is_none());
}

#[test]
fn sdk_adapter_captures_the_entered_caller_runtime() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");

    runtime.block_on(async {
        assert!(CallerRuntime::try_capture().is_some());
    });
}
