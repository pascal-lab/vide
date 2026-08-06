pub(crate) mod preproc;

use preproc::preproc_source_target_at_offset;
#[cfg(test)]
use preproc::{
    ambiguous_preproc_source_targets, push_unique_preproc_hit, syntax_tokens_for_preproc_hit,
};
use preproc_expand::{
    db::PreprocDb,
    macro_file::{Origin, SourceEmittedTokenId},
};
use syntax::{
    SyntaxNode, SyntaxNodeExt, SyntaxTokenWithParent, TokenKind, has_text_range::HasTextRange,
};
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

#[derive(Debug, Clone)]
pub(crate) enum SourceTargetResolution<'tree> {
    Resolved(SourceTarget<'tree>),
    Ambiguous(SourceTargetAlternatives<'tree>),
    Blocked(SourceTargetBlock),
}

impl<'tree> SourceTargetResolution<'tree> {
    pub(crate) fn resolved(self) -> Option<SourceTarget<'tree>> {
        match self {
            Self::Resolved(selection) => Some(selection),
            Self::Ambiguous(SourceTargetAlternatives { .. }) => None,
            Self::Blocked(SourceTargetBlock { .. }) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SourceTarget<'tree> {
    pub range: TextRange,
    pub tokens: Vec<SyntaxTokenWithParent<'tree>>,
}

impl<'tree> SourceTarget<'tree> {
    fn normal_syntax(range: TextRange, tokens: Vec<SyntaxTokenWithParent<'tree>>) -> Self {
        Self { range, tokens }
    }

    fn preproc(range: TextRange, tokens: Vec<SyntaxTokenWithParent<'tree>>) -> Self {
        Self { range, tokens }
    }

    pub(crate) fn into_parts(self) -> (TextRange, Vec<SyntaxTokenWithParent<'tree>>) {
        (self.range, self.tokens)
    }

    pub(crate) fn into_tokens(self) -> Vec<SyntaxTokenWithParent<'tree>> {
        self.into_parts().1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceTargetDomain {
    Preproc,
}

#[derive(Debug, Clone)]
pub(crate) struct SourceTargetAlternatives<'tree> {
    pub domain: SourceTargetDomain,
    pub range: TextRange,
    pub reason: SourceTargetAmbiguity,
    pub targets: Vec<SourceTarget<'tree>>,
}

impl<'tree> SourceTargetAlternatives<'tree> {
    fn preproc_ambiguous(
        range: TextRange,
        hit_count: usize,
        targets: Vec<SourceTarget<'tree>>,
    ) -> Self {
        Self {
            domain: SourceTargetDomain::Preproc,
            range,
            reason: SourceTargetAmbiguity::PreprocHits { hit_count },
            targets,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceTargetAmbiguity {
    PreprocHits { hit_count: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceTargetBlock {
    pub domain: SourceTargetDomain,
    pub range: TextRange,
    pub reason: SourceTargetBlockReason,
}

impl SourceTargetBlock {
    fn preproc_unavailable(range: TextRange) -> Self {
        Self {
            domain: SourceTargetDomain::Preproc,
            range,
            reason: SourceTargetBlockReason::Unavailable,
        }
    }

    fn preproc_ambiguous(range: TextRange, hits: Vec<PreprocTokenHit>) -> Self {
        Self {
            domain: SourceTargetDomain::Preproc,
            range,
            reason: SourceTargetBlockReason::Ambiguous { hits },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceTargetBlockReason {
    Unavailable,
    Ambiguous { hits: Vec<PreprocTokenHit> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreprocTokenHit {
    pub emitted_token: SourceEmittedTokenId,
    pub source_range: TextRange,
    pub origin: Origin,
}

pub(crate) fn source_target_at_offset<'tree, F>(
    db: &dyn PreprocDb,
    file_id: FileId,
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: F,
    emitted: Option<&preproc::EmittedTokenIndex<'tree>>,
) -> Option<SourceTargetResolution<'tree>>
where
    F: Fn(TokenKind) -> usize,
{
    match preproc_source_target_at_offset(db, file_id, root, offset, &precedence, emitted) {
        SourceTargetProviderResult::NotApplicable => {
            normal_syntax_source_target_at_offset(root, offset, &precedence).into_resolution()
        }
        result => result.into_resolution(),
    }
}

fn normal_syntax_source_target_at_offset<'tree>(
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: &impl Fn(TokenKind) -> usize,
) -> SourceTargetProviderResult<'tree> {
    let Some(token) = root.token_at_offset(offset).pick_best_token(precedence) else {
        return SourceTargetProviderResult::NotApplicable;
    };
    let Some(range) = token.text_range() else {
        return SourceTargetProviderResult::NotApplicable;
    };
    SourceTargetProviderResult::Resolved(SourceTarget::normal_syntax(range, vec![token]))
}

enum SourceTargetProviderResult<'tree> {
    Resolved(SourceTarget<'tree>),
    Ambiguous(SourceTargetAlternatives<'tree>),
    Blocked(SourceTargetBlock),
    NotApplicable,
}

impl<'tree> SourceTargetProviderResult<'tree> {
    fn into_resolution(self) -> Option<SourceTargetResolution<'tree>> {
        match self {
            Self::Resolved(selection) => Some(SourceTargetResolution::Resolved(selection)),
            Self::Ambiguous(alternatives) => Some(SourceTargetResolution::Ambiguous(alternatives)),
            Self::Blocked(block) => Some(SourceTargetResolution::Blocked(block)),
            Self::NotApplicable => None,
        }
    }
}

#[cfg(test)]
mod tests;
