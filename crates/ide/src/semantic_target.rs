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
        SourceTarget, SourceTargetAlternatives, SourceTargetAmbiguity, SourceTargetBlock,
        SourceTargetBlockReason, SourceTargetDomain, SourceTargetResolution,
        source_target_at_offset,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetIntent {
    Describe,
    Navigate,
    FindReferences,
    Highlight,
    Rename,
}

impl TargetIntent {
    fn capability(self) -> TargetCapability {
        match self {
            TargetIntent::Describe => TargetCapability::DESCRIBE,
            TargetIntent::Navigate => TargetCapability::NAVIGATE,
            TargetIntent::FindReferences => TargetCapability::REFERENCES,
            TargetIntent::Highlight => TargetCapability::HIGHLIGHT,
            TargetIntent::Rename => TargetCapability::RENAME,
        }
    }
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
    Resolved(TargetCandidate<'tree>),
    Ambiguous(TargetAlternatives<'tree>),
    Blocked(TargetBlock),
    Unresolved,
}

impl<'tree> TargetResolution<'tree> {
    pub(crate) fn unique_for_intent(self, intent: TargetIntent) -> Option<SemanticTarget<'tree>> {
        let mut targets = self.targets_for_intent(intent);
        (targets.len() == 1).then(|| targets.pop().expect("single target should exist"))
    }

    pub(crate) fn targets_for_intent(self, intent: TargetIntent) -> Vec<SemanticTarget<'tree>> {
        let required = intent.capability();
        match self {
            TargetResolution::Resolved(candidate) => {
                candidate.into_target(required).into_iter().collect()
            }
            TargetResolution::Ambiguous(alternatives) => alternatives.into_targets(required),
            TargetResolution::Blocked(block) => {
                let TargetBlock { anchor, reason } = block;
                let _ = (anchor, reason);
                Vec::new()
            }
            TargetResolution::Unresolved => Vec::new(),
        }
    }

    pub(crate) fn from_source_resolution(
        file_id: FileId,
        resolution: SourceTargetResolution<'tree>,
    ) -> Self {
        match resolution {
            SourceTargetResolution::Resolved(target) => {
                let capabilities = source_capabilities();
                Self::Resolved(TargetCandidate::new(SemanticTarget::Source(target), capabilities))
            }
            SourceTargetResolution::Ambiguous(alternatives) => {
                Self::from_source_alternatives(file_id, alternatives)
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

    fn from_source_alternatives(
        file_id: FileId,
        alternatives: SourceTargetAlternatives<'tree>,
    ) -> Self {
        let SourceTargetAlternatives { domain, range, reason, targets } = alternatives;
        let anchor =
            TargetAnchor { file_id, range, origin: TargetOrigin::from_source_domain(domain) };
        let reason = TargetAmbiguityReason::from_source(reason);
        let capabilities = source_capabilities();
        let candidates = targets
            .into_iter()
            .map(|target| TargetCandidate::new(SemanticTarget::Source(target), capabilities))
            .collect();
        Self::Ambiguous(TargetAlternatives { anchor, reason, candidates })
    }

    fn from_preproc_macro(target: PreprocMacroTarget) -> Self {
        let capabilities = target.capabilities();
        Self::Resolved(TargetCandidate::new(SemanticTarget::PreprocMacro(target), capabilities))
    }

    fn from_include(includes: Vec<IncludeDirective>) -> Option<Self> {
        includes.first()?;
        Some(Self::Resolved(TargetCandidate::new(
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
                Self::Ambiguous(TargetAlternatives {
                    anchor,
                    reason: TargetAmbiguityReason::PreprocHits { hit_count: hits.len() },
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
    MacroExpansion,
}

impl TargetOrigin {
    fn from_source_domain(domain: SourceTargetDomain) -> Self {
        match domain {
            SourceTargetDomain::Preproc => TargetOrigin::MacroExpansion,
        }
    }

    fn from_source_block(block: &SourceTargetBlock) -> Self {
        Self::from_source_domain(block.domain)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TargetCandidate<'tree> {
    pub target: SemanticTarget<'tree>,
    pub capabilities: TargetCapability,
}

impl<'tree> TargetCandidate<'tree> {
    fn new(target: SemanticTarget<'tree>, capabilities: TargetCapability) -> Self {
        Self { target, capabilities }
    }

    fn into_target(self, required: TargetCapability) -> Option<SemanticTarget<'tree>> {
        let Self { target, capabilities } = self;
        if !capabilities.contains(required) {
            return None;
        }
        Some(target)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TargetAlternatives<'tree> {
    pub anchor: TargetAnchor,
    pub reason: TargetAmbiguityReason,
    pub candidates: Vec<TargetCandidate<'tree>>,
}

impl<'tree> TargetAlternatives<'tree> {
    fn into_targets(self, required: TargetCapability) -> Vec<SemanticTarget<'tree>> {
        let Self { anchor, reason, candidates } = self;
        let _ = (anchor, reason);
        candidates.into_iter().filter_map(|candidate| candidate.into_target(required)).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TargetAmbiguityReason {
    PreprocHits { hit_count: usize },
}

impl TargetAmbiguityReason {
    fn from_source(reason: SourceTargetAmbiguity) -> Self {
        match reason {
            SourceTargetAmbiguity::PreprocHits { hit_count } => {
                TargetAmbiguityReason::PreprocHits { hit_count }
            }
        }
    }
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
    precedence: F,
) -> TargetResolution<'tree>
where
    F: Fn(TokenKind) -> usize,
{
    if let Some(target) = preproc_macro_target_at(db, file_id, offset) {
        return TargetResolution::from_preproc_macro(target);
    }

    if let Some(includes) = include_target_at(db, file_id, offset) {
        return TargetResolution::from_include(includes).unwrap_or(TargetResolution::Unresolved);
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

        let resolution =
            resolve_semantic_target(host.raw_db(), file_id, offset, Some(root), token_precedence);
        assert!(matches!(
            resolution.clone().unique_for_intent(TargetIntent::Describe),
            Some(SemanticTarget::Source(_))
        ));
        assert!(matches!(
            resolution.clone().unique_for_intent(TargetIntent::Rename),
            Some(SemanticTarget::Source(_))
        ));

        let TargetResolution::Resolved(target) = resolution else {
            panic!("source token should resolve");
        };

        assert!(target.capabilities.contains(TargetCapability::DESCRIBE));
        let SemanticTarget::Source(target) = target.target else {
            panic!("source token should resolve as source target");
        };
        assert_eq!(target.range, range);
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

        assert_eq!(ambiguity.reason, TargetAmbiguityReason::PreprocHits { hit_count: 0 });
        assert!(ambiguity.candidates.is_empty());
    }

    #[test]
    fn ambiguous_source_target_alternatives_project_as_candidates() {
        let range = TextRange::new(TextSize::from(1), TextSize::from(4));
        let target_range = TextRange::new(TextSize::from(2), TextSize::from(3));
        let target = crate::source_targets::SourceTarget {
            origin: crate::source_targets::SourceTargetOrigin::NormalSyntax,
            range: target_range,
            tokens: Vec::new(),
        };
        let alternatives = crate::source_targets::SourceTargetAlternatives {
            domain: crate::source_targets::SourceTargetDomain::Preproc,
            range,
            reason: crate::source_targets::SourceTargetAmbiguity::PreprocHits { hit_count: 2 },
            targets: vec![target.clone(), target],
        };

        let resolution = TargetResolution::from_source_resolution(
            FileId(0),
            crate::source_targets::SourceTargetResolution::Ambiguous(alternatives),
        );

        assert!(resolution.clone().unique_for_intent(TargetIntent::Describe).is_none());
        assert_eq!(resolution.clone().targets_for_intent(TargetIntent::Describe).len(), 2);

        let TargetResolution::Ambiguous(alternatives) = resolution else {
            panic!("source alternatives should stay ambiguous");
        };
        assert_eq!(alternatives.anchor.range, range);
        assert_eq!(alternatives.reason, TargetAmbiguityReason::PreprocHits { hit_count: 2 });
        assert_eq!(alternatives.candidates.len(), 2);
    }
}
