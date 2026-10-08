//! Elaborated instance identity.
//!
//! A hierarchical path is slang's name for one instance after elaboration.
//! Vide stores the path; the live compilation answers where it is in source.

use std::fmt;

/// Stable key for one elaborated instance (`top.u0`, `top.u0[1].inner`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HierPath(String);

impl HierPath {
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HierPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for HierPath {
    fn from(path: String) -> Self {
        Self(path)
    }
}
