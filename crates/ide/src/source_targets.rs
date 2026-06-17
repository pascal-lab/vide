use hir::hir_def::macro_file::{MacroFileId, Origin, macro_files_at_offset};
use rustc_hash::FxHashMap;
use syntax::{
    SyntaxNode, SyntaxNodeExt, SyntaxTokenWithParent, TokenKind, has_text_range::HasTextRange,
};
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use crate::db::root_db::RootDb;

mod macro_gate;
mod preproc;

#[cfg(test)]
use macro_gate::source_macro_invocation_may_cover_offset;
use preproc::preproc_source_target_at_offset;
#[cfg(test)]
use preproc::{
    origin_from_syntax_token_origin, push_unique_preproc_hit, syntax_tokens_for_preproc_hit,
};

#[derive(Debug, Clone)]
pub(crate) enum SourceTargetResolution<'tree> {
    Resolved(SourceTarget<'tree>),
    Blocked(SourceTargetBlock),
}

impl<'tree> SourceTargetResolution<'tree> {
    pub(crate) fn resolved(self) -> Option<SourceTarget<'tree>> {
        match self {
            Self::Resolved(selection) => Some(selection),
            Self::Blocked(SourceTargetBlock { .. }) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SourceTarget<'tree> {
    pub origin: SourceTargetOrigin,
    pub range: TextRange,
    pub tokens: Vec<SyntaxTokenWithParent<'tree>>,
}

impl<'tree> SourceTarget<'tree> {
    fn normal_syntax(range: TextRange, tokens: Vec<SyntaxTokenWithParent<'tree>>) -> Self {
        Self { origin: SourceTargetOrigin::NormalSyntax, range, tokens }
    }

    fn preproc(
        range: TextRange,
        hits: Vec<PreprocTokenHit>,
        tokens: Vec<SyntaxTokenWithParent<'tree>>,
    ) -> Self {
        Self { origin: SourceTargetOrigin::Preproc { hits }, range, tokens }
    }

    pub(crate) fn into_parts(self) -> (TextRange, Vec<SyntaxTokenWithParent<'tree>>) {
        let Self { origin, range, tokens } = self;
        match origin {
            SourceTargetOrigin::NormalSyntax => (range, tokens),
            SourceTargetOrigin::Preproc { hits } => {
                let _hit_count = hits.len();
                (range, tokens)
            }
        }
    }

    pub(crate) fn into_tokens(self) -> Vec<SyntaxTokenWithParent<'tree>> {
        self.into_parts().1
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceTargetOrigin {
    NormalSyntax,
    Preproc { hits: Vec<PreprocTokenHit> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceTargetDomain {
    Preproc,
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

#[derive(Debug, Default)]
pub(crate) struct SourceTargetRequestCache {
    macro_files_by_offset: FxHashMap<(FileId, TextSize), Vec<MacroFileId>>,
}

impl SourceTargetRequestCache {
    fn macro_files_at_offset(
        &mut self,
        db: &RootDb,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<MacroFileId> {
        self.macro_files_by_offset
            .entry((file_id, offset))
            .or_insert_with(|| macro_files_at_offset(db, file_id, offset))
            .clone()
    }

    #[cfg(test)]
    fn macro_files_at_offset_with(
        &mut self,
        file_id: FileId,
        offset: TextSize,
        compute: impl FnOnce() -> Vec<MacroFileId>,
    ) -> Vec<MacroFileId> {
        self.macro_files_by_offset.entry((file_id, offset)).or_insert_with(compute).clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreprocTokenHit {
    pub expansion: usize,
    pub call: usize,
    pub emitted_token: usize,
    pub display_range: TextRange,
    pub source_range: TextRange,
    pub origin: Origin,
}

pub(crate) fn source_target_at_offset<'tree, F>(
    db: &RootDb,
    file_id: FileId,
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: F,
) -> Option<SourceTargetResolution<'tree>>
where
    F: Fn(TokenKind) -> usize,
{
    let mut cache = SourceTargetRequestCache::default();
    source_target_at_offset_with_cache(db, file_id, root, offset, precedence, &mut cache)
}

pub(crate) fn source_target_at_offset_with_cache<'tree, F>(
    db: &RootDb,
    file_id: FileId,
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: F,
    cache: &mut SourceTargetRequestCache,
) -> Option<SourceTargetResolution<'tree>>
where
    F: Fn(TokenKind) -> usize,
{
    match preproc_source_target_at_offset(db, file_id, root, offset, &precedence, cache) {
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
    let Some(token) = root.token_at_offset(offset).pick_bext_token(precedence) else {
        return SourceTargetProviderResult::NotApplicable;
    };
    let Some(range) = token.text_range() else {
        return SourceTargetProviderResult::NotApplicable;
    };
    SourceTargetProviderResult::Resolved(SourceTarget::normal_syntax(range, vec![token]))
}

enum SourceTargetProviderResult<'tree> {
    Resolved(SourceTarget<'tree>),
    Blocked(SourceTargetBlock),
    NotApplicable,
}

impl<'tree> SourceTargetProviderResult<'tree> {
    fn into_resolution(self) -> Option<SourceTargetResolution<'tree>> {
        match self {
            Self::Resolved(selection) => Some(SourceTargetResolution::Resolved(selection)),
            Self::Blocked(block) => Some(SourceTargetResolution::Blocked(block)),
            Self::NotApplicable => None,
        }
    }
}

fn covering_range(ranges: &[TextRange]) -> Option<TextRange> {
    let start = ranges.iter().map(|range| range.start()).min()?;
    let end = ranges.iter().map(|range| range.end()).max()?;
    Some(TextRange::new(start, end))
}

#[cfg(test)]
mod tests;
