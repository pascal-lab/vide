//! VLNV (Vendor:Library:Name:Version) parsing and version relations.
//!
//! FuseSoC identifies cores by VLNV: `vendor:library:name:version-revision`.
//! Dependencies specify version constraints like `>=vendor:lib:name:1.2`.

use std::cmp::Ordering;

/// A parsed VLNV identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Vlnv {
    pub vendor: String,
    pub library: String,
    pub name: String,
    pub version: String,
    pub revision: String,
}

impl Vlnv {
    /// Parse a VLNV string like `vendor:library:name:version-revision`.
    ///
    /// The version field is required; revision is optional (defaults to `0`).
    /// For dependency requirements, the version may be preceded by a relation
    /// operator (handled by [`VlnvRequirement::parse`]).
    pub fn parse(s: &str) -> Result<Self, VlnvError> {
        let parts: Vec<&str> = s.splitn(4, ':').collect();
        if parts.len() != 4 {
            return Err(VlnvError::InvalidFormat(s.to_string()));
        }
        let (version, revision) = split_version_revision(parts[3]);
        Ok(Self {
            vendor: parts[0].to_string(),
            library: parts[1].to_string(),
            name: parts[2].to_string(),
            version,
            revision,
        })
    }

    /// The VLN part (vendor:library:name) without version.
    pub fn vln(&self) -> String {
        format!("{}:{}:{}", self.vendor, self.library, self.name)
    }

    /// Full VLNV string.
    pub fn vlnv(&self) -> String {
        if self.revision == "0" || self.revision.is_empty() {
            format!("{}:{}", self.vln(), self.version)
        } else {
            format!("{}:{}-{}", self.vln(), self.version, self.revision)
        }
    }
}

impl std::fmt::Display for Vlnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.vlnv())
    }
}

/// Split `version-revision` into (version, revision).  Revision defaults to
/// `0` if not present.
fn split_version_revision(s: &str) -> (String, String) {
    if let Some((v, r)) = s.rsplit_once('-') {
        (v.to_string(), r.to_string())
    } else {
        (s.to_string(), "0".to_string())
    }
}

/// Version relation operator for dependency constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionRelation {
    /// No constraint (any version).
    Any,
    /// Exact match `==`.
    Equal,
    /// `>=`
    GreaterEqual,
    /// `>`
    Greater,
    /// `<=`
    LessEqual,
    /// `<`
    Less,
}

impl std::fmt::Display for VersionRelation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VersionRelation::Any => Ok(()),
            VersionRelation::Equal => write!(f, "=="),
            VersionRelation::GreaterEqual => write!(f, ">="),
            VersionRelation::Greater => write!(f, ">"),
            VersionRelation::LessEqual => write!(f, "<="),
            VersionRelation::Less => write!(f, "<"),
        }
    }
}

/// A dependency requirement: relation + VLNV.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlnvRequirement {
    pub relation: VersionRelation,
    pub vlnv: Vlnv,
}

impl VlnvRequirement {
    /// Parse a dependency string like `>=vendor:library:name:1.2`.
    ///
    /// Leading whitespace is trimmed.  If no relation prefix is present,
    /// [`VersionRelation::Any`] is assumed.
    pub fn parse(s: &str) -> Result<Self, VlnvError> {
        let s = s.trim();
        let (relation, rest) = parse_relation_prefix(s);
        let vlnv = Vlnv::parse(rest)?;
        Ok(Self { relation, vlnv })
    }

    /// Check if a candidate VLNV satisfies this requirement.
    ///
    /// VLN must match.  Version must satisfy the relation.
    pub fn matches(&self, candidate: &Vlnv) -> bool {
        if self.vlnv.vln() != candidate.vln() {
            return false;
        }
        let cmp = compare_versions(&candidate.version, &self.vlnv.version);
        match self.relation {
            VersionRelation::Any => true,
            VersionRelation::Equal => {
                candidate.version == self.vlnv.version && candidate.revision == self.vlnv.revision
            }
            VersionRelation::GreaterEqual => cmp != Ordering::Less,
            VersionRelation::Greater => cmp == Ordering::Greater,
            VersionRelation::LessEqual => cmp != Ordering::Greater,
            VersionRelation::Less => cmp == Ordering::Less,
        }
    }
}

fn parse_relation_prefix(s: &str) -> (VersionRelation, &str) {
    if let Some(rest) = s.strip_prefix(">=") {
        (VersionRelation::GreaterEqual, rest)
    } else if let Some(rest) = s.strip_prefix("<=") {
        (VersionRelation::LessEqual, rest)
    } else if let Some(rest) = s.strip_prefix("==") {
        (VersionRelation::Equal, rest)
    } else if let Some(rest) = s.strip_prefix(">") {
        (VersionRelation::Greater, rest)
    } else if let Some(rest) = s.strip_prefix("<") {
        (VersionRelation::Less, rest)
    } else {
        (VersionRelation::Any, s)
    }
}

/// Compare two version strings.  Tries numeric comparison for numeric
/// components, falling back to string comparison.
fn compare_versions(a: &str, b: &str) -> Ordering {
    let a_parts: Vec<&str> = a.split('.').collect();
    let b_parts: Vec<&str> = b.split('.').collect();
    for (ap, bp) in a_parts.iter().zip(b_parts.iter()) {
        match (ap.parse::<i64>(), bp.parse::<i64>()) {
            (Ok(an), Ok(bn)) => match an.cmp(&bn) {
                Ordering::Equal => continue,
                ord => return ord,
            },
            _ => match ap.cmp(bp) {
                Ordering::Equal => continue,
                ord => return ord,
            },
        }
    }
    a_parts.len().cmp(&b_parts.len())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VlnvError {
    InvalidFormat(String),
}

impl std::fmt::Display for VlnvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            VlnvError::InvalidFormat(s) => {
                write!(f, "invalid VLNV format: expected vendor:library:name:version, got `{s}`")
            }
        }
    }
}

impl std::error::Error for VlnvError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_vlnv() {
        let v = Vlnv::parse("vendor:lib:name:1.0").unwrap();
        assert_eq!(v.vendor, "vendor");
        assert_eq!(v.library, "lib");
        assert_eq!(v.name, "name");
        assert_eq!(v.version, "1.0");
        assert_eq!(v.revision, "0");
    }

    #[test]
    fn parses_with_revision() {
        let v = Vlnv::parse("vendor:lib:name:1.0-r3").unwrap();
        assert_eq!(v.version, "1.0");
        assert_eq!(v.revision, "r3");
    }

    #[test]
    fn parses_requirement_with_relation() {
        let req = VlnvRequirement::parse(">=vendor:lib:name:1.2").unwrap();
        assert_eq!(req.relation, VersionRelation::GreaterEqual);
        assert_eq!(req.vlnv.vln(), "vendor:lib:name");
    }

    #[test]
    fn parses_requirement_any() {
        let req = VlnvRequirement::parse("vendor:lib:name:1.0").unwrap();
        assert_eq!(req.relation, VersionRelation::Any);
    }

    #[test]
    fn matches_exact() {
        let req = VlnvRequirement::parse("==vendor:lib:name:1.0").unwrap();
        let candidate = Vlnv::parse("vendor:lib:name:1.0").unwrap();
        assert!(req.matches(&candidate));
    }

    #[test]
    fn matches_greater_equal() {
        let req = VlnvRequirement::parse(">=vendor:lib:name:1.0").unwrap();
        assert!(req.matches(&Vlnv::parse("vendor:lib:name:1.0").unwrap()));
        assert!(req.matches(&Vlnv::parse("vendor:lib:name:2.0").unwrap()));
        assert!(!req.matches(&Vlnv::parse("vendor:lib:name:0.9").unwrap()));
    }

    #[test]
    fn matches_any() {
        let req = VlnvRequirement::parse("vendor:lib:name:1.0").unwrap();
        assert!(req.matches(&Vlnv::parse("vendor:lib:name:99.0").unwrap()));
    }

    #[test]
    fn rejects_wrong_vln() {
        let req = VlnvRequirement::parse("vendor:lib:name:1.0").unwrap();
        assert!(!req.matches(&Vlnv::parse("vendor:lib:other:1.0").unwrap()));
    }

    #[test]
    fn compares_versions() {
        assert_eq!(compare_versions("1.0", "1.0"), Ordering::Equal);
        assert_eq!(compare_versions("2.0", "1.0"), Ordering::Greater);
        assert_eq!(compare_versions("1.0", "1.1"), Ordering::Less);
        assert_eq!(compare_versions("1.0.1", "1.0"), Ordering::Greater);
    }
}
