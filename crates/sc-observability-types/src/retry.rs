//! HTTP status classification for export retries.

/// Returns `true` when an HTTP `status` should be retried: 429 (Too Many
/// Requests) or any 5xx server error.
#[must_use]
pub fn is_retryable(status: u16) -> bool {
    status == 429 || (500..=599).contains(&status)
}

#[cfg(test)]
mod tests {
    use super::is_retryable;

    #[test]
    fn too_many_requests_is_retryable() {
        assert!(is_retryable(429));
    }

    #[test]
    fn server_error_is_retryable() {
        assert!(is_retryable(503));
    }

    #[test]
    fn not_found_is_not_retryable() {
        assert!(!is_retryable(404));
    }
}
