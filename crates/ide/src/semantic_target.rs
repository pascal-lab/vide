use hir::preproc::{
    IncludeDirective, MacroDefinition, MacroParamDefinition, MacroParamReferenceDefinitions,
    MacroReferenceDefinitions, include_directives_at, macro_definition_at,
    macro_param_definition_at, macro_param_reference_definitions_at,
    macro_reference_definitions_at,
};
use syntax::{SyntaxNode, TokenKind};
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use crate::{
    db::root_db::RootDb,
    source_targets::{
        SourceTarget, SourceTargetBlock, SourceTargetBlockReason, SourceTargetDomain,
        SourceTargetOrigin, SourceTargetResolution, source_target_at_offset,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetIntent {
    Describe,
    Navigate,
    FindReferences,
    Highlight,
    #[allow(dead_code)]
    Rename,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub(crate) struct TargetCapability: u8 {
        const DESCRIBE = 1 << 0;
        const NAVIGATE = 1 << 1;
        const REFERENCES = 1 << 2;
        const HIGHLIGHT = 1 << 3;
        const RENAME = 1 << 4;
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SemanticTargetResolution<'tree> {
    pub selection: TargetSelection,
    pub target: Option<SemanticTarget<'tree>>,
    #[allow(dead_code)]
    pub alternatives: Vec<SemanticTarget<'tree>>,
    pub status: TargetStatus,
    pub capabilities: TargetCapability,
}

impl<'tree> SemanticTargetResolution<'tree> {
    pub(crate) fn into_target(self, required: TargetCapability) -> Option<SemanticTarget<'tree>> {
        let Self { selection: _selection, target, alternatives: _, status, capabilities } = self;
        if status != TargetStatus::Complete || !capabilities.contains(required) {
            return None;
        }
        target
    }

    pub(crate) fn from_source_resolution(
        file_id: FileId,
        resolution: SourceTargetResolution<'tree>,
    ) -> Self {
        match resolution {
            SourceTargetResolution::Resolved(target) => {
                let origin = TargetOrigin::from_source_origin(&target.origin);
                let range = target.range;
                let capabilities = source_capabilities();
                Self {
                    selection: TargetSelection { file_id, range, origin },
                    target: Some(SemanticTarget::Source(target)),
                    alternatives: Vec::new(),
                    status: TargetStatus::Complete,
                    capabilities,
                }
            }
            SourceTargetResolution::Blocked(block) => {
                let SourceTargetBlock { range, .. } = block.clone();
                let status = TargetStatus::from_source_block(block.clone());
                Self {
                    selection: TargetSelection {
                        file_id,
                        range,
                        origin: TargetOrigin::from_source_block(&block),
                    },
                    target: None,
                    alternatives: Vec::new(),
                    status,
                    capabilities: TargetCapability::empty(),
                }
            }
        }
    }

    fn from_preproc_macro(file_id: FileId, target: PreprocMacroTarget) -> Self {
        let capabilities = target.capabilities();
        Self {
            selection: TargetSelection {
                file_id,
                range: target.range(),
                origin: TargetOrigin::PreprocMacro,
            },
            target: Some(SemanticTarget::PreprocMacro(target)),
            alternatives: Vec::new(),
            status: TargetStatus::Complete,
            capabilities,
        }
    }

    fn from_include(file_id: FileId, includes: Vec<IncludeDirective>) -> Option<Self> {
        let range = includes.first()?.range;
        Some(Self {
            selection: TargetSelection { file_id, range, origin: TargetOrigin::IncludeDirective },
            target: Some(SemanticTarget::Include(includes)),
            alternatives: Vec::new(),
            status: TargetStatus::Complete,
            capabilities: TargetCapability::DESCRIBE | TargetCapability::NAVIGATE,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TargetSelection {
    pub file_id: FileId,
    pub range: TextRange,
    pub origin: TargetOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetOrigin {
    Source,
    MacroExpansion,
    PreprocMacro,
    IncludeDirective,
}

impl TargetOrigin {
    fn from_source_origin(origin: &SourceTargetOrigin) -> Self {
        match origin {
            SourceTargetOrigin::NormalSyntax => TargetOrigin::Source,
            SourceTargetOrigin::Preproc { .. } => TargetOrigin::MacroExpansion,
        }
    }

    fn from_source_block(block: &SourceTargetBlock) -> Self {
        match block.domain {
            SourceTargetDomain::Preproc => TargetOrigin::MacroExpansion,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TargetStatus {
    Complete,
    Ambiguous(TargetAmbiguity),
    Blocked(TargetBlockReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TargetAmbiguity {
    PreprocHits { candidate_count: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TargetBlockReason {
    PreprocUnavailable,
}

impl TargetStatus {
    fn from_source_block(block: SourceTargetBlock) -> Self {
        match (block.domain, block.reason) {
            (SourceTargetDomain::Preproc, SourceTargetBlockReason::Unavailable) => {
                TargetStatus::Blocked(TargetBlockReason::PreprocUnavailable)
            }
            (SourceTargetDomain::Preproc, SourceTargetBlockReason::Ambiguous { hits }) => {
                TargetStatus::Ambiguous(TargetAmbiguity::PreprocHits {
                    candidate_count: hits.len(),
                })
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum SemanticTarget<'tree> {
    Source(SourceTarget<'tree>),
    PreprocMacro(PreprocMacroTarget),
    Include(Vec<IncludeDirective>),
}

#[derive(Debug, Clone)]
pub(crate) enum PreprocMacroTarget {
    ParamDefinition(MacroParamDefinition),
    ParamReference(MacroParamReferenceDefinitions),
    Definition(MacroDefinition),
    Reference(MacroReferenceDefinitions),
}

impl PreprocMacroTarget {
    pub(crate) fn range(&self) -> TextRange {
        match self {
            PreprocMacroTarget::ParamDefinition(definition) => definition.range,
            PreprocMacroTarget::ParamReference(resolution) => resolution.range,
            PreprocMacroTarget::Definition(definition) => definition.name_range,
            PreprocMacroTarget::Reference(resolution) => resolution.range,
        }
    }

    fn capabilities(&self) -> TargetCapability {
        let mut capabilities = TargetCapability::DESCRIBE;
        let has_definitions = match self {
            PreprocMacroTarget::ParamDefinition(_) | PreprocMacroTarget::Definition(_) => true,
            PreprocMacroTarget::ParamReference(resolution) => !resolution.definitions.is_empty(),
            PreprocMacroTarget::Reference(resolution) => !resolution.definitions.is_empty(),
        };
        if has_definitions {
            capabilities |= TargetCapability::NAVIGATE | TargetCapability::REFERENCES;
        }
        capabilities
    }
}

pub(crate) fn resolve_semantic_target<'tree, F>(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
    root: Option<SyntaxNode<'tree>>,
    _intent: TargetIntent,
    precedence: F,
) -> Option<SemanticTargetResolution<'tree>>
where
    F: Fn(TokenKind) -> usize,
{
    if let Some(target) = preproc_macro_target_at(db, file_id, offset) {
        return Some(SemanticTargetResolution::from_preproc_macro(file_id, target));
    }

    if let Some(includes) = include_target_at(db, file_id, offset) {
        return SemanticTargetResolution::from_include(file_id, includes);
    }

    let root = root?;
    source_target_at_offset(db, file_id, root, offset, precedence)
        .map(|resolution| SemanticTargetResolution::from_source_resolution(file_id, resolution))
}

fn preproc_macro_target_at(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<PreprocMacroTarget> {
    if let Ok(Some(definition)) = macro_param_definition_at(db, file_id, offset) {
        return Some(PreprocMacroTarget::ParamDefinition(definition));
    }

    if let Ok(Some(resolution)) = macro_param_reference_definitions_at(db, file_id, offset) {
        if !resolution.definitions.is_empty() {
            return Some(PreprocMacroTarget::ParamReference(resolution));
        }
    }

    if let Ok(Some(definition)) = macro_definition_at(db, file_id, offset) {
        return Some(PreprocMacroTarget::Definition(definition));
    }

    if let Ok(Some(resolution)) = macro_reference_definitions_at(db, file_id, offset) {
        return Some(PreprocMacroTarget::Reference(resolution));
    }

    None
}

fn include_target_at(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<Vec<IncludeDirective>> {
    let includes = include_directives_at(db, file_id, offset).ok()?;
    (!includes.is_empty()).then_some(includes)
}

fn source_capabilities() -> TargetCapability {
    TargetCapability::DESCRIBE
        | TargetCapability::NAVIGATE
        | TargetCapability::REFERENCES
        | TargetCapability::HIGHLIGHT
        | TargetCapability::RENAME
}

#[cfg(test)]
mod tests {
    use hir::{
        base_db::{change::Change, source_root::SourceRoot},
        semantics::Semantics,
    };
    use syntax::token::TokenKindExt;
    use triomphe::Arc;
    use utils::{
        line_index::{TextRange, TextSize},
        lines::LineEnding,
    };
    use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

    use super::*;
    use crate::analysis_host::AnalysisHost;

    fn token_precedence(kind: syntax::TokenKind) -> usize {
        usize::from(kind.name_like())
    }

    fn setup(text: &str, needle: &str) -> (AnalysisHost, FileId, TextSize, TextRange) {
        let file_id = FileId(0);
        let path = VfsPath::new_virtual_path("/test.sv".to_string());
        let mut file_set = FileSet::default();
        file_set.insert(file_id, path);
        let root = SourceRoot::new_local(file_set);

        let mut change = Change::new();
        change.set_roots(vec![root]);
        change.add_changed_file(ChangedFile {
            file_id,
            change_kind: ChangeKind::Create(Arc::from(text), LineEnding::Unix),
        });

        let mut host = AnalysisHost::default();
        host.apply_change(change);

        let start = text.find(needle).expect("needle should exist");
        let range = TextRange::new(
            TextSize::from(start as u32),
            TextSize::from((start + needle.len()) as u32),
        );
        (host, file_id, range.start(), range)
    }

    #[test]
    fn source_token_target_is_complete_and_source_origin() {
        let (host, file_id, offset, range) =
            setup("module m; wire payload_i; endmodule\n", "payload_i");
        let sema = Semantics::new(host.raw_db());
        let parsed = sema.parse_file(file_id);
        let root = parsed.root().expect("test source should parse");

        let target = resolve_semantic_target(
            host.raw_db(),
            file_id,
            offset,
            Some(root),
            TargetIntent::Describe,
            token_precedence,
        )
        .expect("source token target expected");

        assert_eq!(target.selection.range, range);
        assert_eq!(target.selection.origin, TargetOrigin::Source);
        assert_eq!(target.status, TargetStatus::Complete);
        assert!(target.capabilities.contains(TargetCapability::DESCRIBE));
        assert!(matches!(target.target, Some(SemanticTarget::Source(_))));
    }

    #[test]
    fn source_target_block_is_reported_without_syntax_fallback() {
        let block = crate::source_targets::SourceTargetBlock {
            domain: crate::source_targets::SourceTargetDomain::Preproc,
            range: TextRange::new(TextSize::from(1), TextSize::from(4)),
            reason: crate::source_targets::SourceTargetBlockReason::Unavailable,
        };

        let target = SemanticTargetResolution::from_source_resolution(
            FileId(0),
            crate::source_targets::SourceTargetResolution::Blocked(block.clone()),
        );

        assert_eq!(target.selection.range, block.range);
        assert_eq!(target.selection.origin, TargetOrigin::MacroExpansion);
        assert_eq!(target.status, TargetStatus::Blocked(TargetBlockReason::PreprocUnavailable));
        assert!(target.target.is_none());
        assert!(target.capabilities.is_empty());
    }

    #[test]
    fn ambiguous_source_target_block_is_reported_as_ambiguous() {
        let block = crate::source_targets::SourceTargetBlock {
            domain: crate::source_targets::SourceTargetDomain::Preproc,
            range: TextRange::new(TextSize::from(1), TextSize::from(4)),
            reason: crate::source_targets::SourceTargetBlockReason::Ambiguous { hits: Vec::new() },
        };

        let target = SemanticTargetResolution::from_source_resolution(
            FileId(0),
            crate::source_targets::SourceTargetResolution::Blocked(block),
        );

        assert_eq!(
            target.status,
            TargetStatus::Ambiguous(TargetAmbiguity::PreprocHits { candidate_count: 0 })
        );
        assert!(target.target.is_none());
    }
}
