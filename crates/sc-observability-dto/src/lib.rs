//! Neutral wire DTOs for schema and language-binding generation.

use serde::{Deserialize, Serialize};

/// Canonical decimal wire integer, serialized as a string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DecimalDto(pub String);
