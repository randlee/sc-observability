//! Atomic record and serialized-byte admission accounting for the D.6 lifecycle.
//!
//! A lease belongs to its originating budget and releases both credits on drop,
//! including cancellation. D.6 retains it until an admission's terminal outcome.

use crate::lifecycle::{Signal, SignalArray};
use std::sync::{Arc, Condvar, Mutex, PoisonError};

use sc_observability_types::{ErrorContext, Remediation, error_codes, v2::ExportError};

use crate::config::ValidatedTransportBounds;

#[derive(Debug)]
struct Budget {
    records: usize,
    bytes: usize,
    releases: u64,
    by_signal: SignalArray<usize>,
    #[cfg(test)]
    wait_started: Option<Arc<std::sync::Barrier>>,
}

#[derive(Debug)]
struct SharedBudget {
    state: Mutex<Budget>,
    released: Condvar,
    #[cfg(feature = "durable-store")]
    byte_capacity: usize,
}

/// Shared dual budget; reservations either debit both limits or neither.
#[derive(Debug, Clone)]
pub(crate) struct AdmissionCredits(Arc<SharedBudget>);

/// Non-cloneable ownership of one admitted record and its serialized bytes.
#[derive(Debug)]
#[must_use = "retain the lease until the admitted record reaches a terminal outcome"]
pub(crate) struct CreditLease {
    budget: Arc<SharedBudget>,
    bytes: usize,
    signal: Option<Signal>,
}

#[cfg_attr(
    not(any(test, feature = "durable-store")),
    expect(
        dead_code,
        reason = "durable admission accounting; exercised by credit unit tests"
    )
)]
impl AdmissionCredits {
    /// Waits for a release notification; the mutex makes checking and sleeping atomic.
    #[cfg(any(test, feature = "durable-store"))]
    pub(crate) fn wait_for_release(&self, timeout: std::time::Duration) -> bool {
        let budget = self.0.state.lock().unwrap_or_else(PoisonError::into_inner);
        let generation = budget.releases;
        #[cfg(test)]
        if let Some(barrier) = &budget.wait_started {
            barrier.wait();
        }
        let (budget, _) = self
            .0
            .released
            .wait_timeout_while(budget, timeout, |b| b.releases == generation)
            .unwrap_or_else(PoisonError::into_inner);
        budget.releases != generation
    }

    pub(crate) fn new(bounds: &ValidatedTransportBounds) -> Self {
        Self(Arc::new(SharedBudget {
            state: Mutex::new(Budget {
                records: bounds.queue_capacity().get(),
                bytes: bounds.queue_byte_capacity().get(),
                releases: 0,
                by_signal: SignalArray::new([0; 4]),
                #[cfg(test)]
                wait_started: None,
            }),
            released: Condvar::new(),
            #[cfg(feature = "durable-store")]
            byte_capacity: bounds.queue_byte_capacity().get(),
        }))
    }

    /// Reserves one record and its bytes, returning canonical `QueueFull` on saturation.
    #[cfg(test)]
    pub(crate) fn reserve(&self, serialized_bytes: usize) -> Result<CreditLease, ExportError> {
        self.reserve_inner(serialized_bytes, None)
    }

    #[cfg(feature = "durable-store")]
    pub(crate) fn byte_capacity(&self) -> usize {
        self.0.byte_capacity
    }

    #[cfg(feature = "durable-store")]
    pub(crate) fn reserve_for(
        &self,
        signal: Signal,
        bytes: usize,
    ) -> Result<CreditLease, ExportError> {
        self.reserve_inner(bytes, Some(signal))
    }

    fn reserve_inner(
        &self,
        serialized_bytes: usize,
        signal: Option<Signal>,
    ) -> Result<CreditLease, ExportError> {
        let mut budget = self.0.state.lock().unwrap_or_else(PoisonError::into_inner);
        if budget.records == 0 || serialized_bytes > budget.bytes {
            // Construct diagnostics outside the accounting critical section.
            drop(budget);
            return Err(ExportError::QueueFull {
                context: Box::new(ErrorContext::new(
                    error_codes::otlp::OTLP_QUEUE_FULL,
                    "OTLP admission record or byte budget is exhausted",
                    Remediation::recoverable(
                        "wait for admitted records to finish before retrying",
                        ["reduce telemetry production rate"],
                    ),
                )),
            });
        }
        budget.records -= 1;
        budget.bytes -= serialized_bytes;
        if let Some(signal) = signal
            && let Some(count) = budget.by_signal.get_mut(signal)
        {
            *count += 1;
        }
        Ok(CreditLease {
            budget: Arc::clone(&self.0),
            bytes: serialized_bytes,
            signal,
        })
    }
}

impl Drop for CreditLease {
    fn drop(&mut self) {
        let mut budget = self
            .budget
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        // Every lease debited these exact amounts from this same budget once.
        budget.records += 1;
        budget.bytes += self.bytes;
        if let Some(signal) = self.signal
            && let Some(count) = budget.by_signal.get_mut(signal)
        {
            *count -= 1;
        }
        budget.releases = budget.releases.wrapping_add(1);
        self.budget.released.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{OtelConfig, validated_transport_bounds};

    fn credits(records: usize, bytes: usize) -> AdmissionCredits {
        let bounds = validated_transport_bounds(&OtelConfig {
            queue_capacity: Some(records),
            queue_byte_capacity: Some(bytes),
            ..OtelConfig::default()
        })
        .expect("valid queue bounds");
        AdmissionCredits::new(&bounds)
    }

    fn assert_full(result: Result<CreditLease, ExportError>) {
        let error = result.expect_err("saturated admission must fail");
        assert!(matches!(error, ExportError::QueueFull { .. }));
        assert_eq!(error.diagnostic().code, error_codes::otlp::OTLP_QUEUE_FULL);
    }

    #[test]
    fn record_saturation_returns_canonical_error_and_terminal_drop_releases_both() {
        let credits = credits(1, 10);
        let lease = credits.reserve(10).expect("first record fits");
        assert_full(credits.reserve(0));
        drop(lease);
        let _next = credits.reserve(10).expect("both credits were released");
    }

    #[test]
    fn failed_byte_reservation_consumes_neither_budget() {
        let credits = credits(2, 10);
        let first = credits.reserve(8).expect("first record fits");
        assert_full(credits.reserve(3));
        assert_full(credits.reserve(usize::MAX));
        let second = credits
            .reserve(2)
            .expect("failure retained record and byte credits");
        assert_full(credits.reserve(0));
        drop((first, second));
        let _all_bytes = credits.reserve(10).expect("all bytes returned");
        let _second_record = credits.reserve(0).expect("all records returned");
    }

    #[test]
    fn leases_release_only_the_originating_budget() {
        let first = credits(1, 10);
        let second = credits(1, 20);
        let first_lease = first.reserve(10).unwrap();
        let second_lease = second.reserve(20).unwrap();
        drop(first_lease);
        assert_full(second.reserve(0));
        let _renewed = first.reserve(10).unwrap();
        drop(second_lease);
        let _renewed = second.reserve(20).unwrap();
    }

    #[test]
    fn concurrent_reservations_share_one_budget_and_cancel_safely() {
        let credits = credits(1, 10);
        let held = credits.reserve(10).unwrap();
        let worker_credits = credits.clone();
        std::thread::spawn(move || assert_full(worker_credits.reserve(1)))
            .join()
            .unwrap();
        std::thread::spawn(move || drop(held)).join().unwrap();
        let _lease = credits
            .reserve(10)
            .expect("cross-thread terminal release restored budget");
    }
    #[test]
    fn wait_for_release_zero_timeout_returns_false() {
        assert!(!credits(1, 10).wait_for_release(std::time::Duration::ZERO));
    }

    #[test]
    fn wait_for_release_wakes_on_lease_drop() {
        let credits = credits(1, 10);
        let lease = credits.reserve(10).unwrap();
        let barrier = Arc::new(std::sync::Barrier::new(2));
        credits.0.state.lock().unwrap().wait_started = Some(barrier.clone());
        let dropper = std::thread::spawn(move || {
            barrier.wait();
            drop(lease);
        });
        assert!(credits.wait_for_release(std::time::Duration::from_secs(2)));
        dropper.join().unwrap();
        credits.0.state.lock().unwrap().wait_started = None;
        assert!(credits.reserve(10).is_ok());
    }
    #[test]
    #[cfg(feature = "durable-store")]
    fn profile_credit_is_accounted_and_released() {
        let credits = credits(2, 100);
        let lease = credits.reserve_for(Signal::Profiles, 10).unwrap();
        assert_eq!(credits.0.state.lock().unwrap().by_signal, [0, 0, 0, 1]);
        drop(lease);
        assert_eq!(credits.0.state.lock().unwrap().by_signal, [0; 4]);
    }
    #[test]
    fn poisoned_budget_still_releases_a_credit_without_panicking() {
        let credits = credits(1, 10);
        let lease = credits.reserve(10).unwrap();
        let budget = Arc::clone(&credits.0);
        let _ = std::thread::spawn(move || {
            let _held = budget.state.lock().unwrap();
            panic!("simulate an unwinding credit owner");
        })
        .join();
        drop(lease);
        assert!(credits.reserve(10).is_ok());
    }
}
