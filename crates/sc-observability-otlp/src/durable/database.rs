//! Deadline-bounded ownership of the single write connection.
use super::{AdmissionError, TelemetryClientError, context};
use rusqlite::Connection;
use std::{
    ops::{Deref, DerefMut},
    sync::{Condvar, Mutex, MutexGuard, PoisonError, TryLockError},
    time::{Duration, Instant},
};
#[derive(Debug)]
pub(super) struct Database {
    inner: Mutex<Connection>,
    // Serializes observing availability and sleeping, preventing lost wakeups.
    generation: Mutex<u64>,
    available: Condvar,
}
pub(super) struct Guard<'a> {
    owner: &'a Database,
    inner: Option<MutexGuard<'a, Connection>>,
}
impl Deref for Guard<'_> {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        self.inner
            .as_ref()
            .expect("database guard exists until Drop")
    }
}
impl DerefMut for Guard<'_> {
    fn deref_mut(&mut self) -> &mut Connection {
        self.inner
            .as_mut()
            .expect("database guard exists until Drop")
    }
}
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        drop(self.inner.take());
        let mut generation = self
            .owner
            .generation
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *generation = generation.wrapping_add(1);
        self.owner.available.notify_all();
    }
}
impl Database {
    pub(super) fn new(db: Connection) -> Result<Self, TelemetryClientError> {
        // Acquisition gets one bounded budget. SQL never restarts a five-second
        // busy wait per statement; contention with another process is explicit.
        db.busy_timeout(Duration::ZERO)
            .map_err(super::persistence)?;
        Ok(Self {
            inner: Mutex::new(db),
            generation: Mutex::new(0),
            available: Condvar::new(),
        })
    }
    pub(super) fn lock(&self) -> Result<Guard<'_>, TelemetryClientError> {
        self.lock_for(Duration::from_millis(
            crate::constants::STORE_BUSY_TIMEOUT_MS,
        ))
    }
    pub(super) fn try_lock(&self) -> Result<Guard<'_>, TelemetryClientError> {
        self.lock_for(Duration::ZERO)
    }
    pub(super) fn lock_for(&self, timeout: Duration) -> Result<Guard<'_>, TelemetryClientError> {
        let start = Instant::now();
        let mut generation = self
            .generation
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        loop {
            match self.inner.try_lock() {
                Ok(guard) => {
                    return Ok(Guard {
                        owner: self,
                        inner: Some(guard),
                    });
                }
                Err(TryLockError::Poisoned(error)) => {
                    return Ok(Guard {
                        owner: self,
                        inner: Some(error.into_inner()),
                    });
                }
                Err(TryLockError::WouldBlock) => {}
            }
            let remaining = timeout.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                return Err(AdmissionError::StoreUnavailable { context: context(crate::error_codes::DURABLE_LOCK_TIMEOUT, "store connection is busy; retry this operation after the current transaction") }.into());
            }
            let before = *generation;
            (generation, _) = self
                .available
                .wait_timeout_while(generation, remaining, |value| *value == before)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
}
