//! Single audited helper-spawn seam, with process-isolated test injection.
use std::thread::JoinHandle;
#[cfg(test)]
static FAIL_AT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(usize::MAX);
#[cfg(test)]
static ATTEMPT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
static LIVE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub(crate) fn spawn(
    name: &str,
    run: impl FnOnce() + Send + 'static,
) -> std::io::Result<JoinHandle<()>> {
    #[cfg(test)]
    if ATTEMPT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        == FAIL_AT.load(std::sync::atomic::Ordering::SeqCst)
    {
        return Err(std::io::Error::other("injected helper spawn failure"));
    }
    std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            #[cfg(test)]
            let _live = Live::new();
            run();
        })
}
#[cfg(test)]
struct Live;
#[cfg(test)]
impl Live {
    fn new() -> Self {
        let _gate = crate::sync::lock(&LIVE_GATE);
        LIVE.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        LIVE_CHANGED.notify_all();
        Self
    }
}
#[cfg(test)]
impl Drop for Live {
    fn drop(&mut self) {
        let _gate = crate::sync::lock(&LIVE_GATE);
        LIVE.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        LIVE_CHANGED.notify_all();
    }
}
#[cfg(test)]
pub(crate) fn live() -> usize {
    LIVE.load(std::sync::atomic::Ordering::SeqCst)
}
#[cfg(test)]
pub(crate) fn fail_at(at: usize) {
    FAIL_AT.store(at, std::sync::atomic::Ordering::SeqCst);
}

#[cfg(test)]
static LIVE_GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[cfg(test)]
static LIVE_CHANGED: std::sync::Condvar = std::sync::Condvar::new();
#[cfg(test)]
pub(crate) fn wait_live(expected: usize) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut guard = crate::sync::lock(&LIVE_GATE);
    while live() != expected {
        let (next, timeout) = LIVE_CHANGED
            .wait_timeout(
                guard,
                deadline.saturating_duration_since(std::time::Instant::now()),
            )
            .unwrap();
        guard = next;
        assert!(
            !timeout.timed_out() || live() == expected,
            "helper count {}, expected {}",
            live(),
            expected
        );
    }
}
