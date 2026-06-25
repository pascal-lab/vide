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
pub(crate) enum TargetResolution<'tree> {
    Resolved(TargetSet<'tree>),
    Ambiguous(TargetAmbiguity<'tree>),
    Blocked(TargetBlock),
    Unresolved,
}

impl<'tree> TargetResolution<'tree> {
    pub(crate) fn for_hover(self) -> Option<SemanticTarget<'tree>> {
        self.into_primary(TargetCapability::DESCRIBE)
    }

    pub(crate) fn for_navigation(self) -> Option<SemanticTarget<'tree>> {
        self.into_primary(TargetCapability::NAVIGATE)
    }

    pub(crate) fn for_references(self) -> Option<SemanticTarget<'tree>> {
        self.into_primary(TargetCapability::REFERENCES)
    }

    pub(crate) fn for_highlight(self) -> Option<SemanticTarget<'tree>> {
        self.into_primary(TargetCapability::HIGHLIGHT)
    }

    fn into_primary(self, required: TargetCapability) -> Option<SemanticTarget<'tree>> {
        match self {
            TargetResolution::Resolved(set) => set.into_primary(required),
            TargetResolution::Ambiguous(ambiguity) => {
                let TargetAmbiguity { anchor, reason, candidates } = ambiguity;
                let _ = (anchor, reason, candidates);
                None
            }
            TargetResolution::Blocked(block) => {
                let TargetBlock { anchor, reason } = block;
                let _ = (anchor, reason);
                None
            }
            TargetResolution::Unresolved => None,
        }
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
                Self::Resolved(TargetSet::single(
                    TargetAnchor { file_id, range, origin },
                    SemanticTarget::Source(target),
                    capabilities,
                ))
            }
            SourceTargetResolution::Blocked(block) => {
                let SourceTargetBlock { range, .. } = block.clone();
                let anchor = TargetAnchor {
                    file_id,
                    range,
                    origin: TargetOrigin::from_source_block(&block),
                };
                Self::from_source_block(anchor, block)
            }
        }
    }

    fn from_preproc_macro(file_id: FileId, target: PreprocMacroTarget) -> Self {
        let capabilities = target.capabilities();
        Self::Resolved(TargetSet::single(
            TargetAnchor { file_id, range: target.range(), origin: TargetOrigin::PreprocMacro },
            SemanticTarget::PreprocMacro(target),
            capabilities,
        ))
    }

    fn from_include(file_id: FileId, includes: Vec<IncludeDirective>) -> Option<Self> {
        let range = includes.first()?.range;
        Some(Self::Resolved(TargetSet::single(
            TargetAnchor { file_id, range, origin: TargetOrigin::IncludeDirective },
            SemanticTarget::Include(includes),
            TargetCapability::DESCRIBE | TargetCapability::NAVIGATE,
        )))
    }

    fn from_source_block(anchor: TargetAnchor, block: SourceTargetBlock) -> Self {
        match (block.domain, block.reason) {
            (SourceTargetDomain::Preproc, SourceTargetBlockReason::Unavailable) => {
                Self::Blocked(TargetBlock { anchor, reason: TargetBlockReason::PreprocUnavailable })
            }
            (SourceTargetDomain::Preproc, SourceTargetBlockReason::Ambiguous { hits }) => {
                Self::Ambiguous(TargetAmbiguity {
                    anchor,
                    reason: TargetAmbiguityReason::PreprocHits { candidate_count: hits.len() },
                    candidates: Vec::new(),
                })
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TargetAnchor {
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

#[derive(Debug, Clone)]
pub(crate) struct TargetSet<'tree> {
    pub anchor: TargetAnchor,
    pub primary: TargetCandidate<'tree>,
    pub related: Vec<TargetCandidate<'tree>>,
    pub quality: TargetQuality,
}

impl<'tree> TargetSet<'tree> {
    fn single(
        anchor: TargetAnchor,
        target: SemanticTarget<'tree>,
        capabilities: TargetCapability,
    ) -> Self {
        Self {
            anchor,
            primary: TargetCandidate {
                target,
                role: TargetRole::Primary,
                capabilities,
                quality: TargetQuality::Exact,
            },
            related: Vec::new(),
            quality: TargetQuality::Exact,
        }
    }

    fn into_primary(self, required: TargetCapability) -> Option<SemanticTarget<'tree>> {
        let Self { anchor, primary, related, quality } = self;
        let _ = (anchor, related, quality);
        primary.into_target(required)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TargetCandidate<'tree> {
    pub target: SemanticTarget<'tree>,
    pub role: TargetRole,
    pub capabilities: TargetCapability,
    pub quality: TargetQuality,
}

impl<'tree> TargetCandidate<'tree> {
    fn into_target(self, required: TargetCapability) -> Option<SemanticTarget<'tree>> {
        let Self { target, role, capabilities, quality } = self;
        let _ = (role, quality);
        if !capabilities.contains(required) {
            return None;
        }
        Some(target)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetRole {
    Primary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetQuality {
    Exact,
}

#[derive(Debug, Clone)]
pub(crate) struct TargetAmbiguity<'tree> {
    pub anchor: TargetAnchor,
    pub reason: TargetAmbiguityReason,
    pub candidates: Vec<TargetCandidate<'tree>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TargetAmbiguityReason {
    PreprocHits { candidate_count: usize },
}

#[derive(Debug, Clone)]
pub(crate) struct TargetBlock {
    pub anchor: TargetAnchor,
    pub reason: TargetBlockReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TargetBlockReason {
    PreprocUnavailable,
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
) -> TargetResolution<'tree>
where
    F: Fn(TokenKind) -> usize,
{
    if let Some(target) = preproc_macro_target_at(db, file_id, offset) {
        return TargetResolution::from_preproc_macro(file_id, target);
    }

    if let Some(includes) = include_target_at(db, file_id, offset) {
        return TargetResolution::from_include(file_id, includes)
            .unwrap_or(TargetResolution::Unresolved);
    }

    let Some(root) = root else {
        return TargetResolution::Unresolved;
    };
    source_target_at_offset(db, file_id, root, offset, precedence)
        .map(|resolution| TargetResolution::from_source_resolution(file_id, resolution))
        .unwrap_or(TargetResolution::Unresolved)
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

        let resolution = resolve_semantic_target(
            host.raw_db(),
            file_id,
            offset,
            Some(root),
            TargetIntent::Describe,
            token_precedence,
        );
        assert!(matches!(resolution.clone().for_hover(), Some(SemanticTarget::Source(_))));

        let TargetResolution::Resolved(target) = resolution else {
            panic!("source token should resolve");
        };

        assert_eq!(target.anchor.range, range);
        assert_eq!(target.anchor.origin, TargetOrigin::Source);
        assert_eq!(target.quality, TargetQuality::Exact);
        assert!(target.primary.capabilities.contains(TargetCapability::DESCRIBE));
    }

    #[test]
    fn source_target_block_is_reported_without_syntax_fallback() {
        let block = crate::source_targets::SourceTargetBlock {
            domain: crate::source_targets::SourceTargetDomain::Preproc,
            range: TextRange::new(TextSize::from(1), TextSize::from(4)),
            reason: crate::source_targets::SourceTargetBlockReason::Unavailable,
        };

        let resolution = TargetResolution::from_source_resolution(
            FileId(0),
            crate::source_targets::SourceTargetResolution::Blocked(block.clone()),
        );

        let TargetResolution::Blocked(target) = resolution else {
            panic!("unavailable source target should be blocked");
        };

        assert_eq!(target.anchor.range, block.range);
        assert_eq!(target.anchor.origin, TargetOrigin::MacroExpansion);
        assert_eq!(target.reason, TargetBlockReason::PreprocUnavailable);
    }

    #[test]
    fn ambiguous_source_target_block_is_reported_as_ambiguous() {
        let block = crate::source_targets::SourceTargetBlock {
            domain: crate::source_targets::SourceTargetDomain::Preproc,
            range: TextRange::new(TextSize::from(1), TextSize::from(4)),
            reason: crate::source_targets::SourceTargetBlockReason::Ambiguous { hits: Vec::new() },
        };

        let resolution = TargetResolution::from_source_resolution(
            FileId(0),
            crate::source_targets::SourceTargetResolution::Blocked(block),
        );

        let TargetResolution::Ambiguous(ambiguity) = resolution else {
            panic!("conflicting source target should be ambiguous");
        };

        assert_eq!(ambiguity.reason, TargetAmbiguityReason::PreprocHits { candidate_count: 0 });
        assert!(ambiguity.candidates.is_empty());
    }
}
