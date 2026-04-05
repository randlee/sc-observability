use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::IdentityError;

/// Caller-resolved process identity attached to observations and log events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProcessIdentity {
    /// Optional hostname attached to the event or observation.
    pub hostname: Option<String>,
    /// Optional process identifier attached to the event or observation.
    pub pid: Option<u32>,
}

/// Policy describing how process identity is populated at runtime.
pub enum ProcessIdentityPolicy {
    /// Resolve process identity automatically using the default runtime behavior.
    Auto,
    /// Use a fixed caller-supplied identity.
    Fixed {
        /// Fixed hostname value.
        hostname: Option<String>,
        /// Fixed process identifier value.
        pid: Option<u32>,
    },
    /// Delegate identity resolution to a caller-supplied resolver.
    Resolver(Arc<dyn ProcessIdentityResolver>),
}

impl std::fmt::Debug for ProcessIdentityPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => f.write_str("ProcessIdentityPolicy::Auto"),
            Self::Fixed { hostname, pid } => f
                .debug_struct("ProcessIdentityPolicy::Fixed")
                .field("hostname", hostname)
                .field("pid", pid)
                .finish(),
            Self::Resolver(_) => {
                f.write_str("ProcessIdentityPolicy::Resolver(<dyn ProcessIdentityResolver>)")
            }
        }
    }
}

/// Open resolver contract for caller-defined process identity lookup.
pub trait ProcessIdentityResolver: Send + Sync {
    /// Resolves the process identity to attach to emitted records.
    fn resolve(&self) -> Result<ProcessIdentity, IdentityError>;
}
