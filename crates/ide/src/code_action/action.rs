use utils::text_edit::TextRange;

use super::diagnostics::RepairKind;
use crate::{diagnostics::Diagnostic, source_change::SourceChange};

#[derive(Debug, Clone)]
pub enum CodeActionResolveStrategy {
    None,
    All,
    Single { name: String },
}

impl CodeActionResolveStrategy {
    pub fn is_none(&self) -> bool {
        matches!(self, CodeActionResolveStrategy::None)
    }

    pub fn should_resolve(&self, id: CodeActionId) -> bool {
        match self {
            CodeActionResolveStrategy::None => false,
            CodeActionResolveStrategy::All => true,
            CodeActionResolveStrategy::Single { name } => id.name == name,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeActionId {
    /// Stable kind identifier, also used to offer lazy resolution
    /// (`CodeActionResolveStrategy::Single`). A single request may contain
    /// several actions with the same `name` (e.g. `convert_literal_base`'s
    /// per-target-base variants); they are told apart by `CodeAction::ordinal`.
    pub name: &'static str,
    pub kind: CodeActionKind,
    /// Diagnostic repair this action can satisfy when a matching diagnostic is
    /// present.
    pub repair: Option<RepairKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeActionKind {
    QuickFix,
    Generate,
    Refactor,
    RefactorExtract,
    RefactorInline,
    RefactorRewrite,
}

#[derive(Debug, Clone)]
pub struct CodeAction {
    pub id: CodeActionId,
    /// Per-request occurrence index assigned by the collector. Together with
    /// `id.name` this uniquely identifies one offered action, which is what
    /// lazy resolve uses to locate the exact occurrence the user picked
    /// (several same-name actions — e.g. the multiple target bases of one
    /// literal — are otherwise indistinguishable). Stable across resolve
    /// because the engine is deterministic for a given text/revision.
    pub ordinal: u32,
    pub label: String,
    /// Target ranges are used to sort assists: the smaller the target range,
    /// the more specific assist is, and so it should be sorted first.
    pub target: TextRange,
    /// Compute it lazily.
    pub source_change: Option<SourceChange>,
    /// Server diagnostics this action repairs, attached by the engine when
    /// the action's `RepairKind` matches. Quick-fix classification follows
    /// from a non-empty list.
    pub diagnostics: Vec<Diagnostic>,
}
