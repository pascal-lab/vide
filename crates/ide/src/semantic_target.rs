//! Caret-offset resolution to a semantic target.
//!
//! Every IDE feature that works on a position (goto definition, references,
//! hover, rename, highlight) first resolves the offset to a
//! [`SemanticTarget`]: a preprocessor macro definition/parameter/reference,
//! an include directive, or a plain source token. The preprocessor-owned
//! paths resolve through the preproc model; source tokens fall back to the
//! syntax token at the offset.
//!
//! The preproc provider lives in [`preproc`]; this module owns the target
//! vocabulary and the resolution entry points.

pub(crate) mod preproc;

use preproc_expand::{
    db::PreprocDb,
    preproc::{
        IncludeDirective, MacroDefinition, MacroParamDefinition, MacroParamReferenceDefinitions,
        MacroReferenceDefinitions, include_directives_at, macro_definition_at,
        macro_param_definition_at, macro_param_reference_definitions_at,
        macro_reference_definitions_at,
    },
};
use syntax::{
    SyntaxNode, SyntaxNodeExt, SyntaxTokenWithParent, TokenKind, has_text_range::HasTextRange,
};
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use self::preproc::{EmittedTokenIndex, preproc_source_target_at_offset};

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
    /// The offset is preprocessor-owned but cannot be resolved to syntax
    /// tokens. Distinct from `Unresolved`: callers must not fall back to
    /// plain syntax resolution for these offsets.
    Blocked,
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
            TargetResolution::Blocked | TargetResolution::Unresolved => Vec::new(),
        }
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
    pub reason: TargetAmbiguityReason,
    pub candidates: Vec<TargetCandidate<'tree>>,
}

impl<'tree> TargetAlternatives<'tree> {
    fn into_targets(self, required: TargetCapability) -> Vec<SemanticTarget<'tree>> {
        let Self { reason, candidates } = self;
        let _ = reason;
        candidates.into_iter().filter_map(|candidate| candidate.into_target(required)).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TargetAmbiguityReason {
    PreprocHits { hit_count: usize },
}

#[derive(Debug, Clone)]
pub(crate) enum SemanticTarget<'tree> {
    Source(SourceTarget<'tree>),
    PreprocMacro(PreprocMacroTarget),
    Include(Vec<IncludeDirective>),
    Manifest(crate::manifest::ManifestTarget),
}

/// The syntax tokens a caret offset resolves to, with the display range they
/// share. Macro-emitted tokens can map to several tree tokens (a macro can
/// emit the same argument more than once); `tokens` keeps every copy.
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
            capabilities |= TargetCapability::NAVIGATE
                | TargetCapability::REFERENCES
                | TargetCapability::RENAME;
        }
        capabilities
    }
}

pub(crate) fn resolve_semantic_target<'tree, F>(
    db: &dyn PreprocDb,
    file_id: FileId,
    offset: TextSize,
    root: Option<SyntaxNode<'tree>>,
    precedence: F,
) -> TargetResolution<'tree>
where
    F: Fn(TokenKind) -> usize,
{
    resolve_semantic_target_with_emitted(db, file_id, offset, root, precedence, None)
}
/// Resolves a source offset without consulting preprocessor state.
///
/// Callers that have already proved that a file has no preprocessor-owned
/// tokens use this path to avoid four offset-index queries and include lookup
/// for every syntax token.
pub(crate) fn resolve_plain_syntax_target<'tree>(
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: impl Fn(TokenKind) -> usize,
) -> TargetResolution<'tree> {
    normal_syntax_source_target_at_offset(root, offset, &precedence).map_or(
        TargetResolution::Unresolved,
        |target| {
            TargetResolution::Resolved(TargetCandidate::new(
                SemanticTarget::Source(target),
                source_capabilities(),
            ))
        },
    )
}

/// Like [`resolve_semantic_target`], but reuses a prebuilt emitted-token
/// index of `root`'s tree. Callers that resolve many offsets of one tree
/// (the semantic index build) should build the index once with
/// [`emit_token_index`] and pass it here.
pub(crate) fn resolve_semantic_target_with_emitted<'tree, F>(
    db: &dyn PreprocDb,
    file_id: FileId,
    offset: TextSize,
    root: Option<SyntaxNode<'tree>>,
    precedence: F,
    emitted: Option<&EmittedTokenIndex<'tree>>,
) -> TargetResolution<'tree>
where
    F: Fn(TokenKind) -> usize,
{
    if let Some(target) = preproc_macro_target_at(db, file_id, offset) {
        return TargetResolution::from_preproc_macro(target);
    }

    if db.file_kind(file_id).is_project_manifest()
        && let Some(target) = crate::manifest::target_at(db, file_id, offset)
    {
        return TargetResolution::Resolved(TargetCandidate::new(
            SemanticTarget::Manifest(target),
            crate::manifest::target_capabilities(db, target),
        ));
    }

    if let Some(includes) = include_target_at(db, file_id, offset) {
        return TargetResolution::from_include(includes).unwrap_or(TargetResolution::Unresolved);
    }

    let Some(root) = root else {
        return TargetResolution::Unresolved;
    };
    source_target_at_offset(db, file_id, root, offset, precedence, emitted)
        .unwrap_or(TargetResolution::Unresolved)
}

/// Resolves the caret offset to a semantic target, or `None` when the offset
/// is not a resolvable token. Preprocessor-owned offsets (macro definitions,
/// parameters, references, includes, macro-emitted tokens) resolve through
/// the preproc path; everything else falls back to the plain syntax token at
/// the offset.
fn source_target_at_offset<'tree, F>(
    db: &dyn PreprocDb,
    file_id: FileId,
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: F,
    emitted: Option<&EmittedTokenIndex<'tree>>,
) -> Option<TargetResolution<'tree>>
where
    F: Fn(TokenKind) -> usize,
{
    preproc_source_target_at_offset(db, file_id, root, offset, &precedence, emitted).or_else(|| {
        normal_syntax_source_target_at_offset(root, offset, &precedence).map(|target| {
            TargetResolution::Resolved(TargetCandidate::new(
                SemanticTarget::Source(target),
                source_capabilities(),
            ))
        })
    })
}

fn normal_syntax_source_target_at_offset<'tree>(
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: &impl Fn(TokenKind) -> usize,
) -> Option<SourceTarget<'tree>> {
    let token = root.token_at_offset(offset).pick_best_token(precedence)?;
    let range = token.text_range()?;
    Some(SourceTarget::normal_syntax(range, vec![token]))
}

fn preproc_macro_target_at(
    db: &dyn PreprocDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<PreprocMacroTarget> {
    if let Ok(Some(definition)) = macro_param_definition_at(db, file_id, offset) {
        return Some(PreprocMacroTarget::ParamDefinition(definition));
    }

    if let Ok(Some(resolution)) = macro_param_reference_definitions_at(db, file_id, offset)
        && !resolution.definitions.is_empty()
    {
        return Some(PreprocMacroTarget::ParamReference(resolution));
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
    db: &dyn PreprocDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<Vec<IncludeDirective>> {
    let includes = include_directives_at(db, file_id, offset).ok()?;
    (!includes.is_empty()).then_some(includes)
}

pub(crate) fn source_capabilities() -> TargetCapability {
    TargetCapability::DESCRIBE
        | TargetCapability::NAVIGATE
        | TargetCapability::REFERENCES
        | TargetCapability::HIGHLIGHT
        | TargetCapability::RENAME
}

#[cfg(test)]
mod tests;
