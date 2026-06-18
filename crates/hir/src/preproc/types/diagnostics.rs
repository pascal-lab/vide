use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticTarget {
    pub origin: Origin,
    pub file_id: FileId,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticTargetResult {
    pub covered: bool,
    pub target: Option<DiagnosticTarget>,
}

impl DiagnosticTargetResult {
    pub fn uncovered() -> Self {
        Self { covered: false, target: None }
    }

    pub fn covered(target: Option<DiagnosticTarget>) -> Self {
        Self { covered: true, target }
    }
}
