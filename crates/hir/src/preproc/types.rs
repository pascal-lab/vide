use std::collections::BTreeMap;

use preproc::source::{
    SourceEmittedTokenId, SourceEmittedTokenRange, SourceIncludeDirectiveId, SourceMacroCallId,
    SourceMacroArgumentIdentity, SourceMacroBodyIdentity, SourceMacroCallKey,
    SourceMacroDefinitionId, SourceMacroDefinitionKey, SourceMacroExpansionId,
    SourceMacroExpansionKey, SourceMacroReferenceId, SourcePreprocError, SourcePreprocUnavailable,
};
use smol_str::SmolStr;
use utils::{
    line_index::{TextRange, TextSize},
    uniq_vec::UniqVec,
};
use vfs::{FileId, VfsPath};

use crate::base_db::source_db::{
    PreprocSourceMapError, PreprocVirtualOrigin, SourcePreprocQueryError,
};

pub type PreprocResult<T> = Result<T, PreprocError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocError {
    SourceQuery(SourcePreprocQueryError),
    MissingRootSource,
    UnmappedSource {
        buffer_id: u32,
    },
    MismatchedDefinitionRangeFiles {
        event_id: u32,
        directive_file_id: FileId,
        name_file_id: FileId,
    },
    MismatchedReferenceRangeFiles {
        event_id: u32,
        directive_file_id: FileId,
        name_file_id: FileId,
    },
    MissingDefinitionNameRange {
        event_id: u32,
    },
    SourceMap(PreprocSourceMapError),
    Unavailable {
        reason: PreprocUnavailable,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocUnavailable {
    Source(SourcePreprocUnavailable),
    AmbiguousMacroReferenceContexts { contexts: usize },
    AmbiguousMacroExpansionContexts { contexts: usize },
    AmbiguousMacroParamContexts { contexts: usize },
    AmbiguousMacroDefinitionContexts { contexts: usize },
    AmbiguousDiagnosticProvenance { targets: usize },
    AmbiguousIncludeTargets { targets: usize },
    PartialPreprocContextIndex { skipped_models: usize },
    DisplayOnlyVirtualExpansion { path: VfsPath, origin: PreprocVirtualOrigin },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocAvailability {
    Complete,
    Partial,
    Unavailable(PreprocUnavailable),
}

macro_rules! mapped_preproc_id {
    ($name:ident, $core:ty) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name($core);

        impl $name {
            pub fn raw(self) -> usize {
                self.0.raw()
            }
        }

        impl From<$core> for $name {
            fn from(value: $core) -> Self {
                Self(value)
            }
        }
    };
}

mapped_preproc_id!(MacroReferenceId, SourceMacroReferenceId);
mapped_preproc_id!(IncludeDirectiveId, SourceIncludeDirectiveId);
mapped_preproc_id!(MacroCallId, SourceMacroCallId);
mapped_preproc_id!(MacroExpansionId, SourceMacroExpansionId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MacroDefinitionId {
    Source(SourceMacroDefinitionId),
    ConfiguredPredefine { file_id: FileId, range: TextRange },
}

impl From<SourceMacroDefinitionId> for MacroDefinitionId {
    fn from(value: SourceMacroDefinitionId) -> Self {
        Self::Source(value)
    }
}

pub(crate) const CONFIGURED_PREDEFINE_DEFINE_INDEX: usize = usize::MAX;
pub(crate) const CONFIGURED_PREDEFINE_EVENT_ID: u32 = u32::MAX;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappedPreprocSource {
    RealFile { file_id: FileId },
    VirtualFile { file_id: FileId, path: vfs::VfsPath, origin: PreprocVirtualOrigin },
    VirtualDisplay { path: vfs::VfsPath, origin: PreprocVirtualOrigin },
}

impl MappedPreprocSource {
    pub fn file_id(&self) -> Option<FileId> {
        match self {
            Self::RealFile { file_id } | Self::VirtualFile { file_id, .. } => Some(*file_id),
            Self::VirtualDisplay { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroResolution {
    Resolved {
        definition_id: MacroDefinitionId,
        reason: MacroResolutionReason,
        include_chain: Vec<IncludeChainEntry>,
    },
    Undefined,
    Unavailable(PreprocUnavailable),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroResolutionReason {
    VisibleDefinition,
    IncludeGuardIfNDef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroDefinition {
    pub id: MacroDefinitionId,
    pub source: MappedPreprocSource,
    pub capability: PreprocAvailability,
    pub file_id: FileId,
    pub name: SmolStr,
    pub params: Option<Vec<MacroDefinitionParam>>,
    pub body_tokens: Vec<SmolStr>,
    pub define_index: usize,
    pub event_id: u32,
    pub directive_range: TextRange,
    pub name_range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroDefinitionParam {
    pub param_index: usize,
    pub name: Option<SmolStr>,
    pub range: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroParamDefinition {
    pub macro_definition: MacroDefinition,
    pub param_index: usize,
    pub name: SmolStr,
    pub range: TextRange,
    pub param_range: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroParamReference {
    pub macro_definition: MacroDefinition,
    pub source: MappedPreprocSource,
    pub capability: PreprocAvailability,
    pub file_id: FileId,
    pub param_index: usize,
    pub token_index: usize,
    pub name: SmolStr,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroParamReferenceDefinitions {
    pub references: Vec<MacroParamReference>,
    pub range: TextRange,
    pub definitions: Vec<MacroParamDefinition>,
    pub capability: PreprocAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroParamReferences {
    pub references: Vec<MacroParamReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroUsage {
    pub reference_id: MacroReferenceId,
    pub source: MappedPreprocSource,
    pub capability: PreprocAvailability,
    pub file_id: FileId,
    pub name: SmolStr,
    pub usage_index: usize,
    pub directive_range: TextRange,
    pub range: TextRange,
    pub resolution: MacroResolution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroUsageResolution {
    pub capability: PreprocAvailability,
    pub usage: MacroUsage,
    pub definition: MacroDefinition,
    pub definition_provenance: MacroDefinitionProvenance,
    pub include_chain: Vec<IncludeChainEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroDefinitionProvenance {
    pub id: MacroDefinitionId,
    pub source: MappedPreprocSource,
    pub capability: PreprocAvailability,
    pub event_id: u32,
    pub file_id: FileId,
    pub directive_range: TextRange,
    pub name_range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeChainEntry {
    pub include_event_id: u32,
    pub include_file_id: FileId,
    pub include_range: TextRange,
    pub included_file_id: FileId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroReference {
    pub id: MacroReferenceId,
    pub source: MappedPreprocSource,
    pub capability: PreprocAvailability,
    pub file_id: FileId,
    pub name: SmolStr,
    pub directive_range: TextRange,
    pub range: TextRange,
    pub resolution: MacroResolution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroReferenceResolution {
    pub reference: MacroReference,
    pub definition: MacroDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroReferenceDefinitions {
    pub references: Vec<MacroReference>,
    pub range: TextRange,
    pub definitions: Vec<MacroDefinition>,
    pub capability: PreprocAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroCall {
    pub id: MacroCallId,
    pub reference_id: MacroReferenceId,
    pub source: MappedPreprocSource,
    pub capability: PreprocAvailability,
    pub file_id: FileId,
    pub arguments: Vec<MacroArgument>,
    pub directive_range: TextRange,
    pub range: TextRange,
    pub callee: MacroResolution,
    pub expansion: Option<MacroExpansionId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroCallResolution {
    pub call: MacroCall,
    pub definition: MacroDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroArgument {
    pub argument_index: usize,
    pub source: Option<MappedPreprocSource>,
    pub range: Option<TextRange>,
    pub tokens: Vec<SmolStr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroExpansion {
    pub id: MacroExpansionId,
    pub call: MacroCall,
    pub definition_id: Option<MacroDefinitionId>,
    pub definition: MacroExpansionDefinition,
    pub emitted_token_range: SourceEmittedTokenRange,
    pub display_text: String,
    pub display_source: MappedPreprocSource,
    pub display_range: TextRange,
    pub child_calls: Vec<MacroCallId>,
    pub capability: PreprocAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroExpansionDefinition {
    Source(MacroDefinition),
    Builtin { name: SmolStr, capability: PreprocAvailability },
}

impl MacroExpansionDefinition {
    pub fn name(&self) -> &SmolStr {
        match self {
            Self::Source(definition) => &definition.name,
            Self::Builtin { name, .. } => name,
        }
    }

    pub fn capability(&self) -> &PreprocAvailability {
        match self {
            Self::Source(definition) => &definition.capability,
            Self::Builtin { capability, .. } => capability,
        }
    }

    pub fn capability_mut(&mut self) -> &mut PreprocAvailability {
        match self {
            Self::Source(definition) => &mut definition.capability,
            Self::Builtin { capability, .. } => capability,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroExpansionProvenance {
    pub expansion: MacroExpansion,
    pub tokens: Vec<EmittedTokenProvenance>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedTokenProvenance {
    pub token: SourceEmittedTokenId,
    pub text: SmolStr,
    pub display_range: TextRange,
    pub provenance: TokenProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenProvenance {
    SourceToken {
        source: MappedPreprocSource,
        range: TextRange,
    },
    MacroBody {
        identity: MacroBodyTokenIdentity,
        call: MacroCall,
        definition_id: MacroDefinitionId,
        source: MappedPreprocSource,
        range: TextRange,
    },
    MacroArgument {
        identity: MacroArgumentTokenIdentity,
        call: MacroCall,
        argument_index: usize,
        source: MappedPreprocSource,
        range: TextRange,
    },
    Predefine {
        source: MappedPreprocSource,
    },
    Builtin {
        name: SmolStr,
        call: MacroCall,
    },
    TokenPaste {
        call: MacroCall,
    },
    Stringification {
        call: MacroCall,
    },
    Unavailable(PreprocUnavailable),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacroCallIdentity(u32);

impl MacroCallIdentity {
    pub fn raw(self) -> u32 {
        self.0
    }
}

impl From<SourceMacroCallKey> for MacroCallIdentity {
    fn from(value: SourceMacroCallKey) -> Self {
        Self(value.raw())
    }
}

impl From<syntax::PreprocessorTraceMacroCallId> for MacroCallIdentity {
    fn from(value: syntax::PreprocessorTraceMacroCallId) -> Self {
        Self(value.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacroDefinitionIdentity(u32);

impl MacroDefinitionIdentity {
    pub fn raw(self) -> u32 {
        self.0
    }
}

impl From<SourceMacroDefinitionKey> for MacroDefinitionIdentity {
    fn from(value: SourceMacroDefinitionKey) -> Self {
        Self(value.raw())
    }
}

impl From<syntax::PreprocessorTraceMacroDefinitionId> for MacroDefinitionIdentity {
    fn from(value: syntax::PreprocessorTraceMacroDefinitionId) -> Self {
        Self(value.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacroExpansionIdentity(u32);

impl MacroExpansionIdentity {
    pub fn raw(self) -> u32 {
        self.0
    }
}

impl From<SourceMacroExpansionKey> for MacroExpansionIdentity {
    fn from(value: SourceMacroExpansionKey) -> Self {
        Self(value.raw())
    }
}

impl From<syntax::PreprocessorTraceMacroExpansionId> for MacroExpansionIdentity {
    fn from(value: syntax::PreprocessorTraceMacroExpansionId) -> Self {
        Self(value.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacroBodyTokenIdentity {
    pub call: MacroCallIdentity,
    pub definition: MacroDefinitionIdentity,
    pub expansion: MacroExpansionIdentity,
    pub parent_expansion: Option<MacroExpansionIdentity>,
    pub body_token_index: usize,
}

impl From<SourceMacroBodyIdentity> for MacroBodyTokenIdentity {
    fn from(value: SourceMacroBodyIdentity) -> Self {
        Self {
            call: value.call.into(),
            definition: value.definition.into(),
            expansion: value.expansion.into(),
            parent_expansion: value.parent_expansion.map(Into::into),
            body_token_index: value.body_token_index,
        }
    }
}

impl From<syntax::PreprocessorTraceMacroBodyIdentity> for MacroBodyTokenIdentity {
    fn from(value: syntax::PreprocessorTraceMacroBodyIdentity) -> Self {
        Self {
            call: value.call_id.into(),
            definition: value.definition_id.into(),
            expansion: value.expansion_id.into(),
            parent_expansion: value.parent_expansion_id.map(Into::into),
            body_token_index: value.body_token_index as usize,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacroArgumentTokenIdentity {
    pub call: MacroCallIdentity,
    pub definition: MacroDefinitionIdentity,
    pub expansion: MacroExpansionIdentity,
    pub parent_expansion: Option<MacroExpansionIdentity>,
    pub body_token_index: usize,
    pub argument_index: usize,
    pub argument_token_index: usize,
}

impl From<SourceMacroArgumentIdentity> for MacroArgumentTokenIdentity {
    fn from(value: SourceMacroArgumentIdentity) -> Self {
        Self {
            call: value.call.into(),
            definition: value.definition.into(),
            expansion: value.expansion.into(),
            parent_expansion: value.parent_expansion.map(Into::into),
            body_token_index: value.body_token_index,
            argument_index: value.argument_index,
            argument_token_index: value.argument_token_index,
        }
    }
}

impl From<syntax::PreprocessorTraceMacroArgumentIdentity> for MacroArgumentTokenIdentity {
    fn from(value: syntax::PreprocessorTraceMacroArgumentIdentity) -> Self {
        Self {
            call: value.call_id.into(),
            definition: value.definition_id.into(),
            expansion: value.expansion_id.into(),
            parent_expansion: value.parent_expansion_id.map(Into::into),
            body_token_index: value.body_token_index as usize,
            argument_index: value.argument_index as usize,
            argument_token_index: value.argument_token_index as usize,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MacroTokenIdentity {
    Body(MacroBodyTokenIdentity),
    Argument(MacroArgumentTokenIdentity),
}

impl MacroTokenIdentity {
    pub fn from_syntax_provenance(
        provenance: syntax::PreprocessorTraceTokenProvenance,
    ) -> Option<Self> {
        match provenance {
            syntax::PreprocessorTraceTokenProvenance::MacroBody { identity, .. } => {
                Some(Self::Body(identity.into()))
            }
            syntax::PreprocessorTraceTokenProvenance::MacroArgument { identity, .. } => {
                Some(Self::Argument(identity.into()))
            }
            syntax::PreprocessorTraceTokenProvenance::Source { .. }
            | syntax::PreprocessorTraceTokenProvenance::Builtin { .. }
            | syntax::PreprocessorTraceTokenProvenance::TokenPaste { .. }
            | syntax::PreprocessorTraceTokenProvenance::Stringification { .. }
            | syntax::PreprocessorTraceTokenProvenance::Unavailable => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticProvenance {
    SourceToken {
        source: MappedPreprocSource,
        range: TextRange,
    },
    MacroBody {
        call: MacroCall,
        definition_id: MacroDefinitionId,
        source: MappedPreprocSource,
        range: TextRange,
    },
    MacroArgument {
        call: MacroCall,
        argument_index: usize,
        source: MappedPreprocSource,
        range: TextRange,
    },
    VirtualExpansion {
        source: MappedPreprocSource,
        range: TextRange,
    },
    Builtin {
        call: MacroCall,
        name: SmolStr,
    },
    Unavailable(PreprocUnavailable),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroExpansionQuery {
    Available(Box<MacroExpansion>),
    Ambiguous(Vec<MacroExpansion>),
    Unavailable(Box<MacroExpansionUnavailable>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroExpansionUnavailable {
    pub call: MacroCall,
    pub reason: PreprocUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecursiveMacroExpansion {
    pub root_call: MacroCall,
    pub expansions: Vec<MacroExpansion>,
    pub unavailable: Vec<MacroExpansionUnavailable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecursiveMacroExpansionProvenance {
    pub root_call: MacroCall,
    pub expansions: Vec<MacroExpansionProvenance>,
    pub unavailable: Vec<MacroExpansionUnavailable>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MacroDefinitionKey {
    file_id: FileId,
    range_start: TextSize,
    range_end: TextSize,
    name: SmolStr,
}

impl MacroDefinitionKey {
    pub(crate) fn from_definition(definition: &MacroDefinition) -> Self {
        Self {
            file_id: definition.file_id,
            range_start: definition.name_range.start(),
            range_end: definition.name_range.end(),
            name: definition.name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MacroReferenceKey {
    file_id: FileId,
    range_start: TextSize,
    range_end: TextSize,
    name: SmolStr,
}

impl MacroReferenceKey {
    pub(crate) fn from_reference(reference: &MacroReference) -> Self {
        Self {
            file_id: reference.file_id,
            range_start: reference.range.start(),
            range_end: reference.range.end(),
            name: reference.name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct MacroParamDefinitionKey {
    macro_definition: MacroDefinitionKey,
    param_index: usize,
    range_start: TextSize,
    range_end: TextSize,
    name: SmolStr,
}

impl MacroParamDefinitionKey {
    pub(crate) fn from_definition(definition: &MacroParamDefinition) -> Self {
        Self {
            macro_definition: MacroDefinitionKey::from_definition(&definition.macro_definition),
            param_index: definition.param_index,
            range_start: definition.range.start(),
            range_end: definition.range.end(),
            name: definition.name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct MacroParamReferenceKey {
    macro_definition: MacroDefinitionKey,
    param_index: usize,
    file_id: FileId,
    range_start: TextSize,
    range_end: TextSize,
    name: SmolStr,
}

impl MacroParamReferenceKey {
    pub(crate) fn from_reference(reference: &MacroParamReference) -> Self {
        Self {
            macro_definition: MacroDefinitionKey::from_definition(&reference.macro_definition),
            param_index: reference.param_index,
            file_id: reference.file_id,
            range_start: reference.range.start(),
            range_end: reference.range.end(),
            name: reference.name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct InactiveBranchKey {
    file_id: FileId,
    range_start: TextSize,
    range_end: TextSize,
}

impl InactiveBranchKey {
    pub(crate) fn from_branch(branch: &InactiveBranch) -> Self {
        Self {
            file_id: branch.file_id,
            range_start: branch.range.start(),
            range_end: branch.range.end(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MacroReferenceIndex {
    references_by_definition:
        BTreeMap<MacroDefinitionKey, UniqVec<MacroReference, MacroReferenceKey>>,
    definitions_by_reference:
        BTreeMap<MacroReferenceKey, UniqVec<MacroDefinition, MacroDefinitionKey>>,
    issues: Vec<MacroReferenceIndexIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroReferences {
    pub references: Vec<MacroReference>,
    pub status: MacroReferenceIndexStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroReferenceIndexStatus {
    Complete,
    Partial { issues: Vec<MacroReferenceIndexIssue> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroReferenceIndexIssue {
    SkippedModel {
        file_id: FileId,
        error: PreprocError,
    },
    UnavailableReference {
        file_id: FileId,
        reference_id: MacroReferenceId,
        reason: PreprocUnavailable,
    },
}

impl MacroReferenceIndex {
    pub fn references_for(&self, definition: &MacroDefinition) -> Vec<MacroReference> {
        self.references_by_definition
            .get(&MacroDefinitionKey::from_definition(definition))
            .map(|references| references.as_slice().to_vec())
            .unwrap_or_default()
    }

    pub fn definitions_for_reference(
        &self,
        reference: &MacroReference,
    ) -> Option<&[MacroDefinition]> {
        self.definitions_by_reference
            .get(&MacroReferenceKey::from_reference(reference))
            .map(UniqVec::as_slice)
    }

    pub fn status(&self) -> MacroReferenceIndexStatus {
        if self.issues.is_empty() {
            MacroReferenceIndexStatus::Complete
        } else {
            MacroReferenceIndexStatus::Partial { issues: self.issues.clone() }
        }
    }

    pub(super) fn push(&mut self, definition: MacroDefinition, reference: MacroReference) {
        let definition_key = MacroDefinitionKey::from_definition(&definition);
        let references = self.references_by_definition.entry(definition_key).or_default();
        references.push([MacroReferenceKey::from_reference(&reference)], reference.clone());

        let reference_key = MacroReferenceKey::from_reference(&reference);
        let definitions = self.definitions_by_reference.entry(reference_key).or_default();
        definitions.push([MacroDefinitionKey::from_definition(&definition)], definition);
    }

    pub(super) fn push_issue(&mut self, issue: MacroReferenceIndexIssue) {
        if !self.issues.contains(&issue) {
            self.issues.push(issue);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeDirective {
    pub id: IncludeDirectiveId,
    pub source: MappedPreprocSource,
    pub capability: PreprocAvailability,
    pub file_id: FileId,
    pub include_index: usize,
    pub range: TextRange,
    pub target: IncludeTarget,
    pub status: IncludeDirectiveStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InactiveBranch {
    pub source: MappedPreprocSource,
    pub capability: PreprocAvailability,
    pub file_id: FileId,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncludeTarget {
    Literal { path: SmolStr, resolved_file: Option<FileId> },
    Token { raw: SmolStr },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncludeDirectiveStatus {
    Resolved { source: MappedPreprocSource },
    Unresolved,
    Unavailable(PreprocUnavailable),
}

impl From<SourcePreprocQueryError> for PreprocError {
    fn from(value: SourcePreprocQueryError) -> Self {
        Self::SourceQuery(value)
    }
}

impl From<SourcePreprocError> for PreprocError {
    fn from(value: SourcePreprocError) -> Self {
        Self::SourceQuery(SourcePreprocQueryError::Model(value))
    }
}
