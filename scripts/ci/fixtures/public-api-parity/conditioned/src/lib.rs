//! Target-conditioned public API fixture.
//!
//! Without `mutate-*` features the public API is identical on every target
//! while the private implementation differs by platform. Each `mutate-*`
//! feature introduces exactly one Windows-only public API difference that the
//! ADR-022 comparison must report.

/// Public handle whose private storage differs per platform.
#[derive(Debug, Default)]
pub struct Handle {
    #[cfg(windows)]
    raw: u16,
    #[cfg(not(windows))]
    raw: u32,
    #[cfg(all(windows, feature = "mutate-auto-trait"))]
    unsynchronized: *mut u8,
    /// Public field present on every target.
    pub label: String,
    /// Windows-only public field (mutation).
    #[cfg(all(windows, feature = "mutate-field"))]
    pub windows_only_field: u32,
}

impl Handle {
    /// Available everywhere.
    #[must_use]
    pub fn id(&self) -> u64 {
        u64::from(self.raw) + helper()
    }

    /// Windows-only public method (mutation).
    #[cfg(all(windows, feature = "mutate-method"))]
    pub fn windows_only_method(&self) {}

    /// Signature that changes on Windows under `mutate-signature`.
    #[cfg(not(all(windows, feature = "mutate-signature")))]
    pub fn open(path: &str) -> Self {
        Self {
            label: path.to_owned(),
            ..Self::default()
        }
    }

    /// Signature that changes on Windows under `mutate-signature`.
    #[cfg(all(windows, feature = "mutate-signature"))]
    pub fn open(path: &std::path::Path) -> Self {
        Self {
            label: path.display().to_string(),
            ..Self::default()
        }
    }

    /// Generic bound that tightens on Windows under `mutate-bound`.
    #[cfg(not(all(windows, feature = "mutate-bound")))]
    pub fn run<T: Send>(&self, value: T) -> T {
        value
    }

    /// Generic bound that tightens on Windows under `mutate-bound`.
    #[cfg(all(windows, feature = "mutate-bound"))]
    pub fn run<T: Send + Sync>(&self, value: T) -> T {
        value
    }
}

/// Error surface shared by every target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OpenError {
    /// The handle is unavailable on this platform.
    Unsupported,
    /// Windows-only variant (mutation).
    #[cfg(all(windows, feature = "mutate-variant"))]
    AccessDenied,
}

/// Doc-hidden support function present on every target.
#[doc(hidden)]
pub fn __support_everywhere() {}

/// Doc-hidden support function present only on Windows (mutation).
#[doc(hidden)]
#[cfg(all(windows, feature = "mutate-hidden"))]
pub fn __support_windows_only() {}

mod inner {
    /// Reexported on Windows only under `mutate-reexport`.
    #[derive(Debug, Default)]
    pub struct Thing;
}

#[cfg(all(windows, feature = "mutate-reexport"))]
pub use inner::Thing as WindowsAlias;

/// Feature-only Windows difference: present with `transport` but hidden again
/// by `transport-extra`, so neither the default nor the all-features
/// selection can observe it.
#[cfg(all(windows, feature = "transport", not(feature = "transport-extra")))]
pub fn transport_windows_only() {}

/// Transport entry point shared by every target.
#[cfg(feature = "transport")]
pub fn transport() -> Handle {
    Handle::default()
}

#[cfg(windows)]
fn helper() -> u64 {
    1
}

#[cfg(not(windows))]
fn helper() -> u64 {
    let _ = &inner::Thing;
    2
}

#[cfg(windows)]
#[allow(dead_code)]
fn windows_private_helper() -> &'static str {
    let _ = &inner::Thing;
    "windows"
}

/// A public trait whose blanket implementation is deliberately target-conditioned.
pub trait PlatformExtension {
    fn platform_extension(&self) {}
}

#[cfg(all(windows, feature = "mutate-blanket"))]
impl<T> PlatformExtension for T {}
