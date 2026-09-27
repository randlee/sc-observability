//! Atomic record and serialized-byte admission accounting for the D.6 lifecycle.
//!
//! A lease belongs to its originating budget and releases both credits on drop,
//! including cancellation. D.6 retains it until an admission's terminal outcome.
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "D.21 contract staged for D.6 lifecycle integration"
    )
)]

use std::sync::{Arc, Mutex};

use sc_observability_types::{ErrorContext, Remediation, error_codes, v2::ExportError};

use crate::config::ValidatedTransportBounds;

#[derive(Debug)]
struct Budget {
    records: usize,
    bytes: usize,
}

/// Shared dual budget; reservations either debit both limits or neither.
#[derive(Debug, Clone)]
pub(crate) struct AdmissionCredits(Arc<Mutex<Budget>>);

/// Non-cloneable ownership of one admitted record and its serialized bytes.
#[derive(Debug)]
#[must_use = "retain the lease until the admitted record reaches a terminal outcome"]
pub(crate) struct CreditLease {
    budget: Arc<Mutex<Budget>>,
    bytes: usize,
}

impl AdmissionCredits {
    pub(crate) fn new(bounds: &ValidatedTransportBounds) -> Self {
        Self(Arc::new(Mutex::new(Budget {
            records: bounds.queue_capacity().get(),
            bytes: bounds.queue_byte_capacity().get(),
        })))
    }

    /// Reserves one record and its bytes, returning canonical `QueueFull` on saturation.
    pub(crate) fn reserve(&self, serialized_bytes: usize) -> Result<CreditLease, ExportError> {
        let mut budget = self
            .0
            .lock()
            .expect("credit accounting cannot panic while locked");
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
        Ok(CreditLease {
            budget: Arc::clone(&self.0),
            bytes: serialized_bytes,
        })
    }
}

impl Drop for CreditLease {
    fn drop(&mut self) {
        let mut budget = self
            .budget
            .lock()
            .expect("credit accounting cannot panic while locked");
        // Every lease debited these exact amounts from this same budget once.
        budget.records += 1;
        budget.bytes += self.bytes;
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
}
