use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Stable identifier for an element row in the database. Wraps a UUID so the
/// type system distinguishes "an element id" from any other UUID we might
/// carry (page id, user id, session id).
#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ElementUuid(pub uuid::Uuid);

impl ElementUuid {
    pub fn new_v4() -> Self {
        Self(uuid::Uuid::new_v4())
    }
    pub fn nil() -> Self {
        Self(uuid::Uuid::nil())
    }
    pub fn into_inner(self) -> uuid::Uuid {
        self.0
    }
}

impl std::fmt::Display for ElementUuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl FromStr for ElementUuid {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(uuid::Uuid::parse_str(s)?))
    }
}

slotmap::new_key_type! {
    /// Arena-local handle for a [`Node`]. A [`NodeId`] carries a generation
    /// number, so a stale handle returns `None` from [`crate::Tree::get`]
    /// rather than aliasing a recycled slot.
    pub struct NodeId;
}
