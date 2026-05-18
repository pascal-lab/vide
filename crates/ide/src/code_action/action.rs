use utils::text_edit::TextRange;

use crate::{code_action::RepairKind, source_change::SourceChange};

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

    pub fn should_add(&self, id: CodeActionId) -> bool {
        match self {
            CodeActionResolveStrategy::All | CodeActionResolveStrategy::None => false,
            CodeActionResolveStrategy::Single { name } => id.name == name,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeActionId {
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

impl CodeActionKind {
    pub fn contains(self, other: CodeActionKind) -> bool {
        if self == other {
            return true;
        }

        match self {
            CodeActionKind::Generate => true,
            CodeActionKind::Refactor => matches!(
                other,
                CodeActionKind::RefactorExtract
                    | CodeActionKind::RefactorInline
                    | CodeActionKind::RefactorRewrite
            ),
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CodeAction {
    pub id: CodeActionId,
    pub label: String,
    /// Target ranges are used to sort assists: the smaller the target range,
    /// the more specific assist is, and so it should be sorted first.
    pub target: TextRange,
    /// Compute it lazily.
    pub source_change: Option<SourceChange>,
}
