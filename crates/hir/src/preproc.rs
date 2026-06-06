use std::collections::BTreeMap;

use preproc::source::{
    CapabilityStatus, MacroIncludeTarget, PreprocSourceId, SourceEmittedTokenId,
    SourceEmittedTokenRange, SourceIncludeChainEntry, SourceIncludeDirectiveId,
    SourceIncludeStatus, SourceMacroCall as SourceMacroCallFact, SourceMacroCallId,
    SourceMacroCallStatus as SourceMacroCallStatusFact,
    SourceMacroDefinition as SourceMacroDefinitionFact, SourceMacroDefinitionId,
    SourceMacroExpansion as SourceMacroExpansionFact, SourceMacroExpansionId,
    SourceMacroExpansionQuery as SourceMacroExpansionQueryFact,
    SourceMacroExpansionStatus as SourceMacroExpansionStatusFact,
    SourceMacroParam as SourceMacroParamFact, SourceMacroReference as SourceMacroReferenceFact,
    SourceMacroReferenceId, SourceMacroReferenceSite,
    SourceMacroResolution as SourceMacroResolutionFact,
    SourceMacroResolutionReason as SourceMacroResolutionReasonFact, SourcePreprocError,
    SourcePreprocUnavailable, SourceRange, SourceTokenProvenance as SourceTokenProvenanceFact,
};
use rustc_hash::FxHashSet;
use smol_str::SmolStr;
use utils::{
    line_index::{TextRange, TextSize},
    uniq_vec::UniqVec,
};
use vfs::FileId;

use crate::base_db::{
    project::{CompilationProfileId, Predefine},
    source_db::{
        MappedSourcePreprocModel, PreprocSourceMapError, PreprocSourceMapping,
        PreprocVirtualOrigin, SourceFileKind, SourcePreprocQueryError, SourceRootDb,
    },
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

const CONFIGURED_PREDEFINE_DEFINE_INDEX: usize = usize::MAX;
const CONFIGURED_PREDEFINE_EVENT_ID: u32 = u32::MAX;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappedPreprocSource {
    RealFile { file_id: FileId },
    VirtualFile { file_id: FileId, path: vfs::VfsPath, origin: PreprocVirtualOrigin },
}

impl MappedPreprocSource {
    pub fn file_id(&self) -> FileId {
        match self {
            Self::RealFile { file_id } | Self::VirtualFile { file_id, .. } => *file_id,
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
    pub define_index: usize,
    pub event_id: u32,
    pub directive_range: TextRange,
    pub name_range: TextRange,
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
    pub directive_range: TextRange,
    pub range: TextRange,
    pub callee: MacroResolution,
    pub expansion: Option<MacroExpansionId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroExpansion {
    pub id: MacroExpansionId,
    pub call: MacroCall,
    pub definition_id: MacroDefinitionId,
    pub emitted_token_range: SourceEmittedTokenRange,
    pub virtual_source: MappedPreprocSource,
    pub virtual_range: TextRange,
    pub child_calls: Vec<MacroCallId>,
    pub capability: PreprocAvailability,
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
    pub virtual_range: TextRange,
    pub provenance: TokenProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenProvenance {
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
    Predefine {
        source: MappedPreprocSource,
    },
    Builtin {
        name: SmolStr,
    },
    Unavailable(PreprocUnavailable),
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
    Unavailable(PreprocUnavailable),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroExpansionQuery {
    Available(MacroExpansion),
    Ambiguous(Vec<MacroExpansion>),
    Unavailable(MacroExpansionUnavailable),
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct MacroDefinitionKey {
    file_id: FileId,
    range_start: TextSize,
    range_end: TextSize,
    name: SmolStr,
}

impl MacroDefinitionKey {
    fn from_definition(definition: &MacroDefinition) -> Self {
        Self {
            file_id: definition.file_id,
            range_start: definition.name_range.start(),
            range_end: definition.name_range.end(),
            name: definition.name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct MacroReferenceKey {
    file_id: FileId,
    range_start: TextSize,
    range_end: TextSize,
    name: SmolStr,
}

impl MacroReferenceKey {
    fn from_reference(reference: &MacroReference) -> Self {
        Self {
            file_id: reference.file_id,
            range_start: reference.range.start(),
            range_end: reference.range.end(),
            name: reference.name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MacroReferenceIndex {
    references_by_definition: BTreeMap<MacroDefinitionKey, Vec<MacroReference>>,
    definitions_by_reference: BTreeMap<MacroReferenceKey, Vec<MacroDefinition>>,
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
            .cloned()
            .unwrap_or_default()
    }

    pub fn definitions_for_reference(
        &self,
        reference: &MacroReference,
    ) -> Option<&[MacroDefinition]> {
        self.definitions_by_reference
            .get(&MacroReferenceKey::from_reference(reference))
            .map(Vec::as_slice)
    }

    pub fn status(&self) -> MacroReferenceIndexStatus {
        if self.issues.is_empty() {
            MacroReferenceIndexStatus::Complete
        } else {
            MacroReferenceIndexStatus::Partial { issues: self.issues.clone() }
        }
    }

    fn push(&mut self, definition: MacroDefinition, reference: MacroReference) {
        let definition_key = MacroDefinitionKey::from_definition(&definition);
        let references = self.references_by_definition.entry(definition_key).or_default();
        push_unique_macro_reference(references, reference.clone());

        let reference_key = MacroReferenceKey::from_reference(&reference);
        let definitions = self.definitions_by_reference.entry(reference_key).or_default();
        push_unique_macro_definition(definitions, definition);
    }

    fn push_issue(&mut self, issue: MacroReferenceIndexIssue) {
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

pub fn visible_macros_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<MacroDefinition>> {
    let mut definitions = Vec::new();
    let mut first_error = None;
    for model_file_id in source_preproc_query_model_file_ids(db, file_id) {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };

        for position in mapped.source_map.source_positions_for_file_offset(file_id, offset) {
            for definition in mapped.model.visible_macros_at(position) {
                match map_macro_definition(mapped, definition) {
                    Ok(definition) => push_unique_macro_definition(&mut definitions, definition),
                    Err(error) => record_first_error(&mut first_error, error),
                }
            }
        }
    }

    if definitions.is_empty()
        && let Some(error) = first_error
    {
        return Err(error);
    }

    Ok(definitions)
}

pub fn visible_macro_names_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<SmolStr>> {
    let mut names = UniqVec::<SmolStr, SmolStr>::default();
    for definition in visible_macros_at(db, file_id, offset)? {
        names.push_unique(definition.name.clone());
    }
    for name in configured_predefine_names(db, file_id) {
        names.push_unique(name);
    }

    Ok(names.into_vec())
}

fn configured_predefine_names(db: &dyn SourceRootDb, file_id: FileId) -> Vec<SmolStr> {
    let mut names = UniqVec::<SmolStr, SmolStr>::default();

    let profile_id = db.file_compilation_profile(file_id);
    for predefine in &db.project_config().preprocess_for_profile(profile_id).predefines {
        if let Some(name) = predefine_macro_name(predefine.as_str()) {
            names.push_unique(name);
        }
    }

    for predefine in &db.file_preprocess_config(file_id).predefines {
        if let Some(name) = predefine_macro_name(predefine.as_str()) {
            names.push_unique(name);
        }
    }

    names.into_vec()
}

fn predefine_macro_name(predefine: &str) -> Option<SmolStr> {
    let name = predefine.split_once('=').map_or(predefine, |(name, _)| name);
    let name = name.trim().strip_prefix('`').unwrap_or(name.trim());
    if name.is_empty() { None } else { Some(SmolStr::new(name)) }
}

fn configured_predefine_definitions_for_name(
    db: &dyn SourceRootDb,
    context_file_id: FileId,
    name: &SmolStr,
) -> Vec<MacroDefinition> {
    let mut definitions = Vec::new();
    let profile_id = db.file_compilation_profile(context_file_id);
    let project_preprocess = db.project_config().preprocess_for_profile(profile_id);
    for predefine in &project_preprocess.predefines {
        if let Some(definition) = configured_predefine_definition(db, predefine, name) {
            push_unique_macro_definition(&mut definitions, definition);
        }
    }
    for predefine in &db.file_preprocess_config(context_file_id).predefines {
        if let Some(definition) = configured_predefine_definition(db, predefine, name) {
            push_unique_macro_definition(&mut definitions, definition);
        }
    }
    definitions
}

fn configured_predefine_definitions_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> Vec<MacroDefinition> {
    let mut definitions = Vec::new();
    for context_file_id in source_preproc_query_model_file_ids(db, file_id) {
        let profile_id = db.file_compilation_profile(context_file_id);
        let project_preprocess = db.project_config().preprocess_for_profile(profile_id);
        for predefine in &project_preprocess.predefines {
            if let Some(definition) =
                configured_predefine_definition_at(db, predefine, file_id, offset)
            {
                push_unique_macro_definition(&mut definitions, definition);
            }
        }
        for predefine in &db.file_preprocess_config(context_file_id).predefines {
            if let Some(definition) =
                configured_predefine_definition_at(db, predefine, file_id, offset)
            {
                push_unique_macro_definition(&mut definitions, definition);
            }
        }
    }
    definitions
}

fn configured_predefine_definition_at(
    db: &dyn SourceRootDb,
    predefine: &Predefine,
    file_id: FileId,
    offset: TextSize,
) -> Option<MacroDefinition> {
    let definition =
        configured_predefine_definition(db, predefine, &predefine_macro_name(predefine.as_str())?)?;
    (definition.file_id == file_id && range_contains_offset(definition.name_range, offset))
        .then_some(definition)
}

fn configured_predefine_definition(
    db: &dyn SourceRootDb,
    predefine: &Predefine,
    name: &SmolStr,
) -> Option<MacroDefinition> {
    let predefine_name = predefine_macro_name(predefine.as_str())?;
    if &predefine_name != name {
        return None;
    }
    let source = predefine.source.as_ref()?;
    let file_id = file_id_for_predefine_source_path(db, &source.path)?;
    Some(MacroDefinition {
        id: MacroDefinitionId::ConfiguredPredefine { file_id, range: source.range },
        source: MappedPreprocSource::RealFile { file_id },
        capability: PreprocAvailability::Complete,
        file_id,
        name: predefine_name,
        define_index: CONFIGURED_PREDEFINE_DEFINE_INDEX,
        event_id: CONFIGURED_PREDEFINE_EVENT_ID,
        directive_range: source.range,
        name_range: source.range,
    })
}

fn file_id_for_predefine_source_path(
    db: &dyn SourceRootDb,
    path: &utils::paths::AbsPathBuf,
) -> Option<FileId> {
    db.files().iter().copied().find(|file_id| db.file_path(*file_id).as_ref() == Some(path))
}

pub fn macro_definition_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroDefinition>> {
    let mut first_error = None;
    for model_file_id in source_preproc_query_model_file_ids(db, file_id) {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };

        for definition in mapped.model.macro_definitions().iter() {
            let mapped_definition = map_macro_definition(mapped, definition)?;
            if mapped_definition.file_id == file_id
                && range_contains_offset(mapped_definition.name_range, offset)
            {
                return Ok(Some(mapped_definition));
            }
        }
    }

    let mut configured_definitions = configured_predefine_definitions_at(db, file_id, offset);
    match configured_definitions.len() {
        0 => {}
        1 => return Ok(configured_definitions.pop()),
        contexts => {
            return Err(PreprocError::Unavailable {
                reason: PreprocUnavailable::AmbiguousMacroDefinitionContexts { contexts },
            });
        }
    }

    if let Some(error) = first_error {
        return Err(error);
    }

    Ok(None)
}

pub fn macro_param_definition_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroParamDefinition>> {
    let mut definitions = macro_param_definitions_at(db, file_id, offset)?;
    match definitions.len() {
        0 => Ok(None),
        1 => Ok(definitions.pop()),
        contexts => Err(PreprocError::Unavailable {
            reason: PreprocUnavailable::AmbiguousMacroParamContexts { contexts },
        }),
    }
}

pub fn macro_param_definitions_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<MacroParamDefinition>> {
    let mut definitions = Vec::new();
    let mut first_error = None;

    for model_file_id in source_preproc_query_model_file_ids(db, file_id) {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };

        for definition in mapped.model.macro_definitions().iter() {
            let Some(params) = &definition.params else {
                continue;
            };
            for (param_index, param) in params.iter().enumerate() {
                let Some(param_definition) =
                    map_macro_param_definition(mapped, definition, param_index, param)?
                else {
                    continue;
                };
                if param_definition.macro_definition.file_id == file_id
                    && range_contains_offset(param_definition.range, offset)
                {
                    push_unique_macro_param_definition(&mut definitions, param_definition);
                }
            }
        }
    }

    if definitions.is_empty()
        && let Some(error) = first_error
    {
        return Err(error);
    }

    Ok(definitions)
}

pub fn macro_param_reference_definitions_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroParamReferenceDefinitions>> {
    let mut definitions = Vec::new();
    let mut references = Vec::new();
    let mut query_range = None;
    let mut first_error = None;

    for model_file_id in source_preproc_query_model_file_ids(db, file_id) {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };

        for definition in mapped.model.macro_definitions().iter() {
            let Some(params) = &definition.params else {
                continue;
            };
            for (token_index, token) in definition.body_tokens.iter().enumerate() {
                let Some(token_range) = token.range else {
                    continue;
                };
                let (_, range) =
                    match mapped_source_range_at_offset(mapped, token_range, file_id, offset) {
                        Ok(Some(hit)) => hit,
                        Ok(None) => continue,
                        Err(error) => {
                            record_first_error(&mut first_error, error);
                            continue;
                        }
                    };

                for (param_index, param) in params.iter().enumerate() {
                    if param.name.as_ref() != Some(&token.value) {
                        continue;
                    }
                    let Some(param_definition) =
                        map_macro_param_definition(mapped, definition, param_index, param)?
                    else {
                        continue;
                    };
                    let reference = map_macro_param_reference(
                        mapped,
                        definition,
                        param_index,
                        token_index,
                        token_range,
                    )?;
                    query_range.get_or_insert(range);
                    push_unique_macro_param_definition(&mut definitions, param_definition);
                    push_unique_macro_param_reference(&mut references, reference);
                }
            }
        }
    }

    let Some(range) = query_range else {
        if let Some(error) = first_error {
            return Err(error);
        }
        return Ok(None);
    };

    Ok(Some(MacroParamReferenceDefinitions {
        capability: macro_param_reference_context_capability(&references),
        references,
        range,
        definitions,
    }))
}

pub fn macro_usage_resolution_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroUsageResolution>> {
    let mut resolutions = macro_usage_resolutions_at(db, file_id, offset)?;
    match resolutions.len() {
        0 => Ok(None),
        1 => Ok(resolutions.pop()),
        contexts => Err(PreprocError::Unavailable {
            reason: PreprocUnavailable::AmbiguousMacroReferenceContexts { contexts },
        }),
    }
}

pub fn macro_usage_resolutions_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<MacroUsageResolution>> {
    let mut resolutions = Vec::new();
    let mut first_error = None;
    let mut unavailable_contexts = 0;

    for model_file_id in source_preproc_query_model_file_ids(db, file_id) {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };

        for reference in mapped.model.macro_references().iter() {
            let SourceMacroReferenceSite::Usage { usage_index } = reference.site else {
                continue;
            };
            match mapped_source_range_contains_offset(mapped, reference.name_range, file_id, offset)
            {
                Ok(true) => {}
                Ok(false) => continue,
                Err(error) => {
                    record_first_error(&mut first_error, error);
                    continue;
                }
            }

            let SourceMacroResolutionFact::Resolved { definition, include_chain, .. } =
                &reference.resolution
            else {
                if let SourceMacroResolutionFact::Unavailable(reason) = &reference.resolution {
                    unavailable_contexts += 1;
                    record_first_error(&mut first_error, unavailable_error(reason.clone()));
                }
                continue;
            };
            let mapped_reference = map_macro_reference(mapped, reference)?;
            let definition_fact =
                mapped.model.macro_definitions().get(*definition).ok_or_else(|| {
                    PreprocError::SourceQuery(SourcePreprocQueryError::Model(
                        SourcePreprocError::MissingEvent { event_id: reference.event_id.raw() },
                    ))
                })?;
            let definition = map_macro_definition(mapped, definition_fact)?;
            let definition_provenance =
                map_definition_provenance_from_definition(mapped, definition_fact)?;
            let include_chain = map_include_chain(mapped, include_chain)?;

            push_unique_macro_usage_resolution(
                &mut resolutions,
                MacroUsageResolution {
                    usage: MacroUsage {
                        reference_id: mapped_reference.id,
                        source: mapped_reference.source,
                        capability: mapped_reference.capability.clone(),
                        file_id: mapped_reference.file_id,
                        name: mapped_reference.name,
                        usage_index,
                        directive_range: mapped_reference.directive_range,
                        range: mapped_reference.range,
                        resolution: mapped_reference.resolution,
                    },
                    definition,
                    definition_provenance,
                    include_chain,
                },
            );
        }
    }

    if !resolutions.is_empty() {
        return Ok(resolutions);
    }
    if unavailable_contexts > 1 {
        return Err(PreprocError::Unavailable {
            reason: PreprocUnavailable::AmbiguousMacroReferenceContexts {
                contexts: unavailable_contexts,
            },
        });
    }
    if let Some(error) = first_error {
        return Err(error);
    }

    Ok(Vec::new())
}

pub fn macro_reference_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroReference>> {
    let Some(mut contexts) = macro_reference_definitions_at(db, file_id, offset)? else {
        return Ok(None);
    };
    if contexts.references.len() == 1 {
        return Ok(contexts.references.pop());
    }
    Err(PreprocError::Unavailable {
        reason: PreprocUnavailable::AmbiguousMacroReferenceContexts {
            contexts: contexts.references.len(),
        },
    })
}

pub fn macro_reference_resolution_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroReferenceResolution>> {
    let Some(mut resolution) = macro_reference_definitions_at(db, file_id, offset)? else {
        return Ok(None);
    };
    if resolution.references.len() != 1 || resolution.definitions.len() != 1 {
        return Err(PreprocError::Unavailable {
            reason: PreprocUnavailable::AmbiguousMacroReferenceContexts {
                contexts: resolution.references.len().max(resolution.definitions.len()),
            },
        });
    }
    let reference = resolution.references.pop().unwrap();
    let Some(definition) = resolution.definitions.into_iter().next() else {
        return Ok(None);
    };
    Ok(Some(MacroReferenceResolution { reference, definition }))
}

pub fn macro_reference_definitions_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroReferenceDefinitions>> {
    let mut definitions = Vec::new();
    let mut references = Vec::new();
    let mut query_range = None;
    let mut first_error = None;

    for model_file_id in source_preproc_query_model_file_ids(db, file_id) {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };

        for reference in mapped.model.macro_references().iter() {
            let (_, range) = match mapped_source_range_at_offset(
                mapped,
                reference.name_range,
                file_id,
                offset,
            ) {
                Ok(Some(hit)) => hit,
                Ok(None) => continue,
                Err(error) => {
                    record_first_error(&mut first_error, error);
                    continue;
                }
            };
            query_range.get_or_insert(range);

            let mapped_reference = match map_macro_reference(mapped, reference) {
                Ok(reference) => reference,
                Err(error) => {
                    record_first_error(&mut first_error, error);
                    continue;
                }
            };
            push_unique_macro_reference_context(&mut references, mapped_reference.clone());

            match &reference.resolution {
                SourceMacroResolutionFact::Resolved { definition, .. } => {
                    let Some(definition) = mapped.model.macro_definitions().get(*definition) else {
                        record_first_error(
                            &mut first_error,
                            PreprocError::SourceQuery(SourcePreprocQueryError::Model(
                                SourcePreprocError::MissingEvent {
                                    event_id: reference.event_id.raw(),
                                },
                            )),
                        );
                        continue;
                    };
                    let definition = match map_macro_definition(mapped, definition) {
                        Ok(definition) => definition,
                        Err(error) => {
                            record_first_error(&mut first_error, error);
                            continue;
                        }
                    };

                    push_unique_macro_definition(&mut definitions, definition);
                }
                SourceMacroResolutionFact::Undefined => {
                    for definition in configured_predefine_definitions_for_name(
                        db,
                        model_file_id,
                        &mapped_reference.name,
                    ) {
                        push_unique_macro_definition(&mut definitions, definition);
                    }
                }
                SourceMacroResolutionFact::Unavailable(_) => {}
            }
        }
    }

    let Some(range) = query_range else {
        if let Some(error) = first_error {
            return Err(error);
        }
        return Ok(None);
    };

    Ok(Some(MacroReferenceDefinitions {
        capability: macro_reference_context_capability(&references),
        references,
        range,
        definitions,
    }))
}

pub fn immediate_macro_expansion_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroExpansionQuery>> {
    let mut queries = macro_expansion_queries_at(db, file_id, offset)?;
    let available = queries
        .iter()
        .filter_map(|query| match query {
            MacroExpansionQuery::Available(expansion) => Some(expansion.clone()),
            MacroExpansionQuery::Ambiguous(expansions) => Some(expansions.first()?.clone()),
            MacroExpansionQuery::Unavailable(_) => None,
        })
        .collect::<Vec<_>>();
    if available.len() > 1 {
        return Ok(Some(MacroExpansionQuery::Ambiguous(available)));
    }
    if available.len() == 1 {
        return Ok(Some(MacroExpansionQuery::Available(available.into_iter().next().unwrap())));
    }
    match queries.len() {
        0 => Ok(None),
        1 => Ok(queries.pop()),
        contexts => Err(PreprocError::Unavailable {
            reason: PreprocUnavailable::AmbiguousMacroExpansionContexts { contexts },
        }),
    }
}

pub fn macro_expansion_queries_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<MacroExpansionQuery>> {
    let mut queries = Vec::new();
    let mut first_error = None;

    for model_file_id in source_preproc_query_model_file_ids(db, file_id) {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };
        let Some(call_fact) = source_macro_call_at(mapped, file_id, offset) else {
            continue;
        };
        let query = immediate_macro_expansion_for_call(mapped, call_fact)?;
        push_unique_macro_expansion_query(&mut queries, query);
    }

    if !queries.is_empty() {
        return Ok(queries);
    }
    if let Some(error) = first_error {
        return Err(error);
    }

    Ok(Vec::new())
}

pub fn recursive_macro_expansion_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<RecursiveMacroExpansion>> {
    let expansions = recursive_macro_expansions_at(db, file_id, offset)?;
    match expansions.len() {
        0 => Ok(None),
        1 => Ok(expansions.into_iter().next()),
        contexts => Err(PreprocError::Unavailable {
            reason: PreprocUnavailable::AmbiguousMacroExpansionContexts { contexts },
        }),
    }
}

pub fn recursive_macro_expansions_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<RecursiveMacroExpansion>> {
    let mut expansions = Vec::new();
    let mut first_error = None;

    for model_file_id in source_preproc_query_model_file_ids(db, file_id) {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };
        let Some(call_fact) = source_macro_call_at(mapped, file_id, offset) else {
            continue;
        };
        let recursive = recursive_macro_expansion_for_call(mapped, call_fact)?;
        push_unique_recursive_macro_expansion(&mut expansions, recursive);
    }

    if !expansions.is_empty() {
        return Ok(expansions);
    }
    if let Some(error) = first_error {
        return Err(error);
    }

    Ok(Vec::new())
}

pub fn macro_expansion_provenance_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroExpansionProvenance>> {
    let provenances = macro_expansion_provenances_at(db, file_id, offset)?;
    match provenances.len() {
        0 => Ok(None),
        1 => Ok(provenances.into_iter().next()),
        contexts => Err(PreprocError::Unavailable {
            reason: PreprocUnavailable::AmbiguousMacroExpansionContexts { contexts },
        }),
    }
}

pub fn macro_expansion_provenances_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<MacroExpansionProvenance>> {
    let mut provenances = Vec::new();
    let mut first_error = None;
    for model_file_id in source_preproc_query_model_file_ids(db, file_id) {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };
        let Some(call_fact) = source_macro_call_at(mapped, file_id, offset) else {
            continue;
        };
        if let Some(provenance) = macro_expansion_provenance_for_call(mapped, call_fact)? {
            push_unique_macro_expansion_provenance(&mut provenances, provenance);
        }
    }

    if !provenances.is_empty() {
        return Ok(provenances);
    }
    if let Some(error) = first_error {
        return Err(error);
    }

    Ok(Vec::new())
}

pub fn macro_expansion_provenance_for_range(
    db: &dyn SourceRootDb,
    file_id: FileId,
    range: TextRange,
) -> PreprocResult<Option<MacroExpansionProvenance>> {
    let provenances = macro_expansion_provenances_for_range(db, file_id, range)?;
    match provenances.len() {
        0 => Ok(None),
        1 => Ok(provenances.into_iter().next()),
        contexts => Err(PreprocError::Unavailable {
            reason: PreprocUnavailable::AmbiguousMacroExpansionContexts { contexts },
        }),
    }
}

pub fn macro_expansion_provenances_for_range(
    db: &dyn SourceRootDb,
    file_id: FileId,
    range: TextRange,
) -> PreprocResult<Vec<MacroExpansionProvenance>> {
    let mut provenances = Vec::new();
    let mut first_error = None;
    for model_file_id in source_preproc_query_model_file_ids(db, file_id) {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };
        let Some(call_fact) = source_macro_call_intersecting_range(mapped, file_id, range) else {
            continue;
        };
        if let Some(provenance) = macro_expansion_provenance_for_call(mapped, call_fact)? {
            push_unique_macro_expansion_provenance(&mut provenances, provenance);
        }
    }

    if !provenances.is_empty() {
        return Ok(provenances);
    }
    if let Some(error) = first_error {
        return Err(error);
    }

    Ok(Vec::new())
}

pub fn diagnostic_provenance_for_range(
    db: &dyn SourceRootDb,
    file_id: FileId,
    range: TextRange,
) -> PreprocResult<Option<DiagnosticProvenance>> {
    let mut provenances = Vec::new();
    let mut first_error = None;

    for model_file_id in source_preproc_query_model_file_ids(db, file_id) {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };
        let Some(call_fact) = source_macro_call_intersecting_range(mapped, file_id, range) else {
            continue;
        };
        let provenance = diagnostic_provenance_for_call(mapped, call_fact)?;
        push_unique_diagnostic_provenance(&mut provenances, provenance);
    }

    let precise = provenances
        .iter()
        .filter(|provenance| !matches!(provenance, DiagnosticProvenance::Unavailable(_)))
        .cloned()
        .collect::<Vec<_>>();
    if precise.len() == 1 {
        return Ok(Some(precise.into_iter().next().unwrap()));
    }
    if precise.len() > 1 {
        return Ok(Some(DiagnosticProvenance::Unavailable(
            PreprocUnavailable::AmbiguousDiagnosticProvenance { targets: precise.len() },
        )));
    }
    if provenances.len() == 1 {
        return Ok(provenances.into_iter().next());
    }
    if provenances.len() > 1 {
        return Ok(Some(DiagnosticProvenance::Unavailable(
            PreprocUnavailable::AmbiguousDiagnosticProvenance { targets: provenances.len() },
        )));
    }
    if let Some(error) = first_error {
        return Err(error);
    }

    Ok(None)
}

pub fn macro_references(
    db: &dyn SourceRootDb,
    file_id: FileId,
    definition: &MacroDefinition,
) -> PreprocResult<MacroReferences> {
    let profile_id = db
        .file_compilation_profile(file_id)
        .or_else(|| db.file_compilation_profile(definition.file_id));
    let index = db.macro_reference_index_for_profile(profile_id);
    Ok(MacroReferences { references: index.references_for(definition), status: index.status() })
}

pub fn macro_param_references(
    db: &dyn SourceRootDb,
    file_id: FileId,
    definition: &MacroParamDefinition,
) -> PreprocResult<MacroParamReferences> {
    let profile_id = db
        .file_compilation_profile(file_id)
        .or_else(|| db.file_compilation_profile(definition.macro_definition.file_id));
    let mut references = Vec::new();
    let mut first_error = None;

    for model_file_id in preproc_reference_model_file_ids(db, profile_id) {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };

        for source_definition in mapped.model.macro_definitions().iter() {
            let mapped_definition = match map_macro_definition(mapped, source_definition) {
                Ok(definition) => definition,
                Err(error) => {
                    record_first_error(&mut first_error, error);
                    continue;
                }
            };
            if !same_macro_definition(&mapped_definition, &definition.macro_definition) {
                continue;
            }
            let Some(params) = &source_definition.params else {
                continue;
            };
            let Some(param) = params.get(definition.param_index) else {
                continue;
            };
            if param.name.as_ref() != Some(&definition.name) {
                continue;
            }

            for (token_index, token) in source_definition.body_tokens.iter().enumerate() {
                if param.name.as_ref() != Some(&token.value) {
                    continue;
                }
                let Some(token_range) = token.range else {
                    continue;
                };
                match map_macro_param_reference(
                    mapped,
                    source_definition,
                    definition.param_index,
                    token_index,
                    token_range,
                ) {
                    Ok(reference) => push_unique_macro_param_reference(&mut references, reference),
                    Err(error) => record_first_error(&mut first_error, error),
                }
            }
        }
    }

    if references.is_empty()
        && let Some(error) = first_error
    {
        return Err(error);
    }

    Ok(MacroParamReferences { references })
}

pub(crate) fn build_macro_reference_index(
    db: &dyn SourceRootDb,
    profile_id: Option<CompilationProfileId>,
) -> MacroReferenceIndex {
    let mut index = MacroReferenceIndex::default();

    for model_file_id in preproc_reference_model_file_ids(db, profile_id) {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped.as_ref() {
            Ok(mapped) => mapped,
            Err(error) => {
                index.push_issue(MacroReferenceIndexIssue::SkippedModel {
                    file_id: model_file_id,
                    error: error.clone().into(),
                });
                continue;
            }
        };
        collect_macro_references_in_model(db, mapped, model_file_id, &mut index);
    }

    index
}

fn collect_macro_references_in_model(
    db: &dyn SourceRootDb,
    mapped: &MappedSourcePreprocModel,
    model_file_id: FileId,
    index: &mut MacroReferenceIndex,
) {
    for reference in mapped.model.macro_references().iter() {
        let SourceMacroResolutionFact::Resolved { definition, .. } = reference.resolution else {
            if reference.resolution == SourceMacroResolutionFact::Undefined {
                collect_configured_predefine_reference(db, mapped, model_file_id, reference, index);
                continue;
            }
            if let SourceMacroResolutionFact::Unavailable(reason) = &reference.resolution {
                index.push_issue(MacroReferenceIndexIssue::UnavailableReference {
                    file_id: model_file_id,
                    reference_id: reference.id.into(),
                    reason: PreprocUnavailable::Source(reason.clone()),
                });
            }
            continue;
        };

        let Some(definition) = mapped.model.macro_definitions().get(definition) else {
            index.push_issue(MacroReferenceIndexIssue::SkippedModel {
                file_id: model_file_id,
                error: PreprocError::SourceQuery(SourcePreprocQueryError::Model(
                    SourcePreprocError::MissingEvent { event_id: reference.event_id.raw() },
                )),
            });
            continue;
        };

        let definition = match map_macro_definition(mapped, definition) {
            Ok(definition) => definition,
            Err(error) => {
                index.push_issue(MacroReferenceIndexIssue::SkippedModel {
                    file_id: model_file_id,
                    error,
                });
                continue;
            }
        };
        let reference = match map_macro_reference(mapped, reference) {
            Ok(reference) => reference,
            Err(error) => {
                index.push_issue(MacroReferenceIndexIssue::SkippedModel {
                    file_id: model_file_id,
                    error,
                });
                continue;
            }
        };
        index.push(definition, reference);
    }
}

fn collect_configured_predefine_reference(
    db: &dyn SourceRootDb,
    mapped: &MappedSourcePreprocModel,
    model_file_id: FileId,
    source_reference: &SourceMacroReferenceFact,
    index: &mut MacroReferenceIndex,
) {
    let reference = match map_macro_reference(mapped, source_reference) {
        Ok(reference) => reference,
        Err(error) => {
            index.push_issue(MacroReferenceIndexIssue::SkippedModel {
                file_id: model_file_id,
                error,
            });
            return;
        }
    };
    for definition in configured_predefine_definitions_for_name(db, model_file_id, &reference.name)
    {
        index.push(definition, reference.clone());
    }
}

pub fn include_directive_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<IncludeDirective>> {
    let mut directives = include_directives_at(db, file_id, offset)?;
    match directives.len() {
        0 => Ok(None),
        1 => Ok(directives.pop()),
        targets => Err(PreprocError::Unavailable {
            reason: PreprocUnavailable::AmbiguousIncludeTargets { targets },
        }),
    }
}

pub fn include_directives_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<IncludeDirective>> {
    let mut directives = Vec::new();
    let mut first_error = None;
    for model_file_id in source_preproc_query_model_file_ids(db, file_id) {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };
        for include in mapped.model.include_graph().directives() {
            let Some(target_range) = include.target_range else {
                continue;
            };
            let (source, range) =
                match mapped_source_range_at_offset(mapped, target_range, file_id, offset) {
                    Ok(Some(hit)) => hit,
                    Ok(None) => continue,
                    Err(error) => {
                        record_first_error(&mut first_error, error);
                        continue;
                    }
                };
            let status = map_include_status(mapped, &include.status)?;
            let resolved_file = match &status {
                IncludeDirectiveStatus::Resolved { source } => Some(source.file_id()),
                IncludeDirectiveStatus::Unresolved | IncludeDirectiveStatus::Unavailable(_) => None,
            };
            let target = match &include.target {
                MacroIncludeTarget::Literal { path, .. } => {
                    IncludeTarget::Literal { path: path.clone(), resolved_file }
                }
                MacroIncludeTarget::Token { raw } => IncludeTarget::Token { raw: raw.clone() },
            };
            let directive = IncludeDirective {
                id: include.id.into(),
                source,
                capability: capability_status(&mapped.model.capabilities().include_edges),
                file_id,
                include_index: include.id.raw(),
                range,
                target,
                status,
            };
            push_unique_include_directive(&mut directives, directive);
        }
    }

    if !directives.is_empty() {
        return Ok(directives);
    }
    if let Some(error) = first_error {
        return Err(error);
    }

    Ok(Vec::new())
}

pub fn inactive_branches(
    db: &dyn SourceRootDb,
    file_id: FileId,
) -> PreprocResult<Vec<InactiveBranch>> {
    let mut branches = Vec::new();
    let mut first_error = None;

    for model_file_id in source_preproc_query_model_file_ids(db, file_id) {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };

        for source_range in mapped.model.inactive_ranges() {
            let (source, range) = match map_mapped_source_range(mapped, *source_range) {
                Ok(mapped_range) => mapped_range,
                Err(error) => {
                    record_first_error(&mut first_error, error);
                    continue;
                }
            };
            let branch_file_id = source.file_id();
            if branch_file_id == file_id {
                push_unique_inactive_branch(
                    &mut branches,
                    InactiveBranch {
                        source,
                        capability: capability_status(&mapped.model.capabilities().inactive_ranges),
                        file_id: branch_file_id,
                        range,
                    },
                );
            }
        }
    }

    if branches.is_empty()
        && let Some(error) = first_error
    {
        return Err(error);
    }

    Ok(branches)
}

fn mapped_result(
    result: &Result<MappedSourcePreprocModel, SourcePreprocQueryError>,
) -> PreprocResult<&MappedSourcePreprocModel> {
    result.as_ref().map_err(|err| err.clone().into())
}

fn source_preproc_query_model_file_ids(db: &dyn SourceRootDb, file_id: FileId) -> Vec<FileId> {
    let profile_id = db.file_compilation_profile(file_id);
    let mut file_ids = Vec::new();
    let mut seen = FxHashSet::default();
    push_unique_file_id(&mut file_ids, &mut seen, file_id);
    for model_file_id in preproc_reference_model_file_ids(db, profile_id) {
        push_unique_file_id(&mut file_ids, &mut seen, model_file_id);
    }
    file_ids
}

fn push_unique_file_id(file_ids: &mut Vec<FileId>, seen: &mut FxHashSet<FileId>, file_id: FileId) {
    if seen.insert(file_id) {
        file_ids.push(file_id);
    }
}

fn record_first_error(first_error: &mut Option<PreprocError>, error: PreprocError) {
    if first_error.is_none() {
        *first_error = Some(error);
    }
}

fn map_source_range(
    mapped: &MappedSourcePreprocModel,
    source_range: SourceRange,
) -> PreprocResult<(FileId, TextRange)> {
    let (source, range) = map_mapped_source_range(mapped, source_range)?;
    Ok((source.file_id(), range))
}

fn map_source_id(
    mapped: &MappedSourcePreprocModel,
    source: PreprocSourceId,
) -> PreprocResult<FileId> {
    mapped.source_map.file_id(source).map_err(PreprocError::SourceMap)
}

fn map_mapped_source_range(
    mapped: &MappedSourcePreprocModel,
    source_range: SourceRange,
) -> PreprocResult<(MappedPreprocSource, TextRange)> {
    let range = mapped.source_map.map_range(source_range).map_err(PreprocError::SourceMap)?;
    let source = map_mapped_source_id(mapped, source_range.source)?;
    Ok((source, range))
}

fn mapped_source_range_at_offset(
    mapped: &MappedSourcePreprocModel,
    source_range: SourceRange,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<(MappedPreprocSource, TextRange)>> {
    let (source, range) = map_mapped_source_range(mapped, source_range)?;
    Ok((source.file_id() == file_id && range_contains_offset(range, offset))
        .then_some((source, range)))
}

fn mapped_source_range_contains_offset(
    mapped: &MappedSourcePreprocModel,
    source_range: SourceRange,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<bool> {
    Ok(mapped_source_range_at_offset(mapped, source_range, file_id, offset)?.is_some())
}

fn map_mapped_source_id(
    mapped: &MappedSourcePreprocModel,
    source: PreprocSourceId,
) -> PreprocResult<MappedPreprocSource> {
    match mapped.source_map.get(source) {
        Some(PreprocSourceMapping::RealFile(file_id)) => {
            Ok(MappedPreprocSource::RealFile { file_id: *file_id })
        }
        Some(PreprocSourceMapping::VirtualFile { file_id, path, origin }) => {
            Ok(MappedPreprocSource::VirtualFile {
                file_id: *file_id,
                path: path.clone(),
                origin: origin.clone(),
            })
        }
        Some(PreprocSourceMapping::Unmapped(reason)) => {
            Err(PreprocError::SourceMap(PreprocSourceMapError::UnmappedSource {
                source,
                reason: reason.clone(),
            }))
        }
        None => Err(PreprocError::SourceMap(PreprocSourceMapError::MissingSource { source })),
    }
}

fn map_macro_definition(
    mapped: &MappedSourcePreprocModel,
    definition: &SourceMacroDefinitionFact,
) -> PreprocResult<MacroDefinition> {
    let (mut source, mut directive_range, mut name_range) = map_definition_ranges(
        mapped,
        definition.event_id.raw(),
        definition.directive_range,
        definition.name_range,
    )?;
    if let Some(manifest_source) =
        mapped.source_map.predefine_manifest_source(definition.name_range.source)
    {
        source = MappedPreprocSource::RealFile { file_id: manifest_source.file_id };
        directive_range = manifest_source.range;
        name_range = manifest_source.range;
    }
    Ok(MacroDefinition {
        id: definition.id.into(),
        file_id: source.file_id(),
        source,
        capability: capability_status(&mapped.model.capabilities().definition_name_ranges),
        name: definition.name.clone(),
        define_index: define_index_for_definition(mapped, definition)?,
        event_id: definition.event_id.raw(),
        directive_range,
        name_range,
    })
}

fn map_macro_param_definition(
    mapped: &MappedSourcePreprocModel,
    definition: &SourceMacroDefinitionFact,
    param_index: usize,
    param: &SourceMacroParamFact,
) -> PreprocResult<Option<MacroParamDefinition>> {
    let Some(name) = &param.name else {
        return Ok(None);
    };
    let Some(name_source_range) = param.name_range else {
        return Ok(None);
    };
    let macro_definition = map_macro_definition(mapped, definition)?;
    let (source, range) = map_mapped_source_range(mapped, name_source_range)?;
    if source.file_id() != macro_definition.file_id {
        return Err(PreprocError::MismatchedDefinitionRangeFiles {
            event_id: definition.event_id.raw(),
            directive_file_id: macro_definition.file_id,
            name_file_id: source.file_id(),
        });
    }
    let param_range = param
        .range
        .map(|range| map_mapped_source_range(mapped, range).map(|(_, range)| range))
        .transpose()?;

    Ok(Some(MacroParamDefinition {
        macro_definition,
        param_index,
        name: name.clone(),
        range,
        param_range,
    }))
}

fn map_macro_param_reference(
    mapped: &MappedSourcePreprocModel,
    definition: &SourceMacroDefinitionFact,
    param_index: usize,
    token_index: usize,
    token_range: SourceRange,
) -> PreprocResult<MacroParamReference> {
    let macro_definition = map_macro_definition(mapped, definition)?;
    let (source, range) = map_mapped_source_range(mapped, token_range)?;
    let file_id = source.file_id();
    let name = definition
        .params
        .as_ref()
        .and_then(|params| params.get(param_index))
        .and_then(|param| param.name.clone())
        .ok_or_else(|| {
            PreprocError::SourceQuery(SourcePreprocQueryError::Model(
                SourcePreprocError::MissingEvent { event_id: definition.event_id.raw() },
            ))
        })?;

    Ok(MacroParamReference {
        macro_definition,
        source,
        capability: PreprocAvailability::Complete,
        file_id,
        param_index,
        token_index,
        name,
        range,
    })
}

fn map_definition_provenance_from_definition(
    mapped: &MappedSourcePreprocModel,
    definition: &SourceMacroDefinitionFact,
) -> PreprocResult<MacroDefinitionProvenance> {
    let definition = map_macro_definition(mapped, definition)?;
    Ok(MacroDefinitionProvenance {
        id: definition.id,
        source: definition.source,
        capability: definition.capability,
        event_id: definition.event_id,
        file_id: definition.file_id,
        directive_range: definition.directive_range,
        name_range: definition.name_range,
    })
}

fn map_macro_reference(
    mapped: &MappedSourcePreprocModel,
    reference: &SourceMacroReferenceFact,
) -> PreprocResult<MacroReference> {
    let (source, directive_range, name_range) = map_reference_ranges(mapped, reference)?;
    Ok(MacroReference {
        id: reference.id.into(),
        file_id: source.file_id(),
        source,
        capability: capability_status(&mapped.model.capabilities().macro_reference_resolution),
        name: reference.name.clone(),
        directive_range,
        range: name_range,
        resolution: map_macro_resolution(mapped, &reference.resolution)?,
    })
}

fn map_macro_call(
    mapped: &MappedSourcePreprocModel,
    call: &SourceMacroCallFact,
) -> PreprocResult<MacroCall> {
    let (source, range) = map_mapped_source_range(mapped, call.call_range)?;
    Ok(MacroCall {
        id: call.id.into(),
        reference_id: call.reference.into(),
        file_id: source.file_id(),
        source,
        capability: macro_call_availability(&call.status),
        directive_range: range,
        range,
        callee: map_macro_resolution(mapped, &call.callee)?,
        expansion: call.expansion.map(Into::into),
    })
}

fn map_macro_expansion(
    mapped: &MappedSourcePreprocModel,
    expansion: &SourceMacroExpansionFact,
) -> PreprocResult<MacroExpansion> {
    let Some(call) = mapped.model.macro_calls().get(expansion.call) else {
        return Err(PreprocError::Unavailable {
            reason: PreprocUnavailable::Source(SourcePreprocUnavailable::MissingMacroCall {
                call: expansion.call,
            }),
        });
    };
    Ok(MacroExpansion {
        id: expansion.id.into(),
        call: map_macro_call(mapped, call)?,
        definition_id: expansion.definition.into(),
        emitted_token_range: expansion.emitted_token_range,
        virtual_source: map_expansion_virtual_source(mapped, expansion.id)?,
        virtual_range: mapped
            .source_map
            .emitted_token_range(expansion.id, expansion.emitted_token_range)
            .map_err(PreprocError::SourceMap)?,
        child_calls: expansion.child_calls.iter().copied().map(Into::into).collect(),
        capability: macro_expansion_availability(&expansion.status),
    })
}

fn map_expansion_virtual_source(
    mapped: &MappedSourcePreprocModel,
    expansion: SourceMacroExpansionId,
) -> PreprocResult<MappedPreprocSource> {
    match mapped.source_map.expansion_source(expansion).map_err(PreprocError::SourceMap)? {
        PreprocSourceMapping::VirtualFile { file_id, path, origin } => {
            Ok(MappedPreprocSource::VirtualFile { file_id, path, origin })
        }
        PreprocSourceMapping::RealFile(file_id) => Ok(MappedPreprocSource::RealFile { file_id }),
        PreprocSourceMapping::Unmapped(reason) => {
            Err(PreprocError::Unavailable { reason: PreprocUnavailable::Source(reason) })
        }
    }
}

fn source_macro_call_at(
    mapped: &MappedSourcePreprocModel,
    file_id: FileId,
    offset: TextSize,
) -> Option<&SourceMacroCallFact> {
    mapped.model.macro_calls().iter().find(|call| {
        let Ok((source, range)) = map_mapped_source_range(mapped, call.call_range) else {
            return false;
        };
        source.file_id() == file_id && range_contains_offset(range, offset)
    })
}

fn source_macro_call_intersecting_range(
    mapped: &MappedSourcePreprocModel,
    file_id: FileId,
    source_range: TextRange,
) -> Option<&SourceMacroCallFact> {
    mapped.model.macro_calls().iter().find(|call| {
        let Ok((source, range)) = map_mapped_source_range(mapped, call.call_range) else {
            return false;
        };
        source.file_id() == file_id && range.intersect(source_range).is_some()
    })
}

fn immediate_macro_expansion_for_call(
    mapped: &MappedSourcePreprocModel,
    call_fact: &SourceMacroCallFact,
) -> PreprocResult<MacroExpansionQuery> {
    let call = map_macro_call(mapped, call_fact)?;
    Ok(match mapped.model.immediate_macro_expansion(call_fact.id) {
        SourceMacroExpansionQueryFact::Available(expansion) => {
            let Some(expansion) = mapped.model.macro_expansions().get(expansion) else {
                return Ok(MacroExpansionQuery::Unavailable(MacroExpansionUnavailable {
                    call,
                    reason: PreprocUnavailable::Source(
                        SourcePreprocUnavailable::MissingMacroExpansion { call: call_fact.id },
                    ),
                }));
            };
            MacroExpansionQuery::Available(map_macro_expansion(mapped, expansion)?)
        }
        SourceMacroExpansionQueryFact::Unavailable(reason) => {
            MacroExpansionQuery::Unavailable(MacroExpansionUnavailable {
                call,
                reason: PreprocUnavailable::Source(reason),
            })
        }
    })
}

fn recursive_macro_expansion_for_call(
    mapped: &MappedSourcePreprocModel,
    call_fact: &SourceMacroCallFact,
) -> PreprocResult<RecursiveMacroExpansion> {
    let root_call = map_macro_call(mapped, call_fact)?;
    let recursive = mapped.model.recursive_macro_expansion(call_fact.id);
    let expansions = recursive
        .expansions
        .into_iter()
        .filter_map(|expansion| mapped.model.macro_expansions().get(expansion))
        .map(|expansion| map_macro_expansion(mapped, expansion))
        .collect::<PreprocResult<Vec<_>>>()?;
    let unavailable = recursive
        .unavailable
        .into_iter()
        .map(|unavailable| {
            let Some(call) = mapped.model.macro_calls().get(unavailable.call) else {
                return Err(PreprocError::Unavailable {
                    reason: PreprocUnavailable::Source(
                        SourcePreprocUnavailable::MissingMacroCall { call: unavailable.call },
                    ),
                });
            };
            Ok(MacroExpansionUnavailable {
                call: map_macro_call(mapped, call)?,
                reason: PreprocUnavailable::Source(unavailable.reason),
            })
        })
        .collect::<PreprocResult<Vec<_>>>()?;

    Ok(RecursiveMacroExpansion { root_call, expansions, unavailable })
}

fn diagnostic_provenance_for_call(
    mapped: &MappedSourcePreprocModel,
    call_fact: &SourceMacroCallFact,
) -> PreprocResult<DiagnosticProvenance> {
    match mapped.model.immediate_macro_expansion(call_fact.id) {
        SourceMacroExpansionQueryFact::Available(_) => {
            let Some(provenance) = macro_expansion_provenance_for_call(mapped, call_fact)? else {
                return Ok(DiagnosticProvenance::Unavailable(PreprocUnavailable::Source(
                    SourcePreprocUnavailable::MissingMacroExpansion { call: call_fact.id },
                )));
            };
            Ok(diagnostic_target_for_expansion(&provenance))
        }
        SourceMacroExpansionQueryFact::Unavailable(reason) => {
            Ok(DiagnosticProvenance::Unavailable(PreprocUnavailable::Source(reason)))
        }
    }
}

fn macro_expansion_provenance_for_call(
    mapped: &MappedSourcePreprocModel,
    call_fact: &SourceMacroCallFact,
) -> PreprocResult<Option<MacroExpansionProvenance>> {
    let SourceMacroExpansionQueryFact::Available(expansion_id) =
        mapped.model.immediate_macro_expansion(call_fact.id)
    else {
        return Ok(None);
    };
    let Some(expansion) = mapped.model.macro_expansions().get(expansion_id) else {
        return Ok(None);
    };
    let expansion = map_macro_expansion(mapped, expansion)?;
    let mut tokens = Vec::new();
    for token_id in emitted_token_ids(expansion.emitted_token_range) {
        let Some(token) = mapped.model.emitted_tokens().get(token_id) else {
            return Err(PreprocError::SourceMap(PreprocSourceMapError::MissingEmittedToken {
                token: token_id,
            }));
        };
        let Some(provenance) = mapped.model.token_provenance().get(token.provenance) else {
            return Err(unavailable_error(
                SourcePreprocUnavailable::TokenProvenanceAuthorityUnavailable,
            ));
        };
        tokens.push(EmittedTokenProvenance {
            token: token_id,
            text: token.text.clone(),
            virtual_range: mapped
                .source_map
                .emitted_token_text_range(expansion_id, token_id)
                .map_err(PreprocError::SourceMap)?,
            provenance: map_token_provenance(mapped, provenance)?,
        });
    }

    Ok(Some(MacroExpansionProvenance { expansion, tokens }))
}

fn emitted_token_ids(range: SourceEmittedTokenRange) -> impl Iterator<Item = SourceEmittedTokenId> {
    let start = range.start.raw();
    let end = start.saturating_add(range.len);
    (start..end).map(SourceEmittedTokenId::new)
}

fn map_token_provenance(
    mapped: &MappedSourcePreprocModel,
    provenance: &SourceTokenProvenanceFact,
) -> PreprocResult<TokenProvenance> {
    Ok(match provenance {
        SourceTokenProvenanceFact::Source { token_range } => {
            let (source, range) = map_mapped_source_range(mapped, *token_range)?;
            TokenProvenance::SourceToken { source, range }
        }
        SourceTokenProvenanceFact::MacroBody { definition, body_token_range, call } => {
            let call = mapped_macro_call(mapped, *call)?;
            let (source, range) = map_mapped_source_range(mapped, *body_token_range)?;
            TokenProvenance::MacroBody { call, definition_id: (*definition).into(), source, range }
        }
        SourceTokenProvenanceFact::MacroArgument { call, argument_index, argument_token_range } => {
            let call = mapped_macro_call(mapped, *call)?;
            let (source, range) = map_mapped_source_range(mapped, *argument_token_range)?;
            TokenProvenance::MacroArgument { call, argument_index: *argument_index, source, range }
        }
        SourceTokenProvenanceFact::TokenPaste { .. }
        | SourceTokenProvenanceFact::Stringification { .. } => TokenProvenance::Unavailable(
            PreprocUnavailable::Source(SourcePreprocUnavailable::UnsupportedEmittedTokenProvenance),
        ),
        SourceTokenProvenanceFact::Predefine { source } => {
            TokenProvenance::Predefine { source: map_mapped_source_id(mapped, *source)? }
        }
        SourceTokenProvenanceFact::Builtin { name } => {
            TokenProvenance::Builtin { name: name.clone() }
        }
        SourceTokenProvenanceFact::Unavailable(reason) => {
            TokenProvenance::Unavailable(PreprocUnavailable::Source(reason.clone()))
        }
    })
}

fn mapped_macro_call(
    mapped: &MappedSourcePreprocModel,
    call: SourceMacroCallId,
) -> PreprocResult<MacroCall> {
    let Some(call) = mapped.model.macro_calls().get(call) else {
        return Err(unavailable_error(SourcePreprocUnavailable::MissingMacroCall { call }));
    };
    map_macro_call(mapped, call)
}

fn diagnostic_target_for_expansion(provenance: &MacroExpansionProvenance) -> DiagnosticProvenance {
    let mut saw_unavailable = None;
    for token in &provenance.tokens {
        match &token.provenance {
            TokenProvenance::SourceToken { source, range } => {
                return DiagnosticProvenance::SourceToken { source: source.clone(), range: *range };
            }
            TokenProvenance::MacroBody { call, definition_id, source, range } => {
                return DiagnosticProvenance::MacroBody {
                    call: call.clone(),
                    definition_id: *definition_id,
                    source: source.clone(),
                    range: *range,
                };
            }
            TokenProvenance::MacroArgument { call, argument_index, source, range } => {
                return DiagnosticProvenance::MacroArgument {
                    call: call.clone(),
                    argument_index: *argument_index,
                    source: source.clone(),
                    range: *range,
                };
            }
            TokenProvenance::Unavailable(reason) => {
                saw_unavailable = Some(reason.clone());
            }
            TokenProvenance::Predefine { .. } | TokenProvenance::Builtin { .. } => {}
        }
    }

    saw_unavailable.map_or_else(
        || DiagnosticProvenance::VirtualExpansion {
            source: provenance.expansion.virtual_source.clone(),
            range: provenance.expansion.virtual_range,
        },
        DiagnosticProvenance::Unavailable,
    )
}

fn map_macro_resolution(
    mapped: &MappedSourcePreprocModel,
    resolution: &SourceMacroResolutionFact,
) -> PreprocResult<MacroResolution> {
    Ok(match resolution {
        SourceMacroResolutionFact::Resolved { definition, reason, include_chain } => {
            MacroResolution::Resolved {
                definition_id: (*definition).into(),
                reason: map_macro_resolution_reason(*reason),
                include_chain: map_include_chain(mapped, include_chain)?,
            }
        }
        SourceMacroResolutionFact::Undefined => MacroResolution::Undefined,
        SourceMacroResolutionFact::Unavailable(reason) => {
            MacroResolution::Unavailable(PreprocUnavailable::Source(reason.clone()))
        }
    })
}

fn map_macro_resolution_reason(reason: SourceMacroResolutionReasonFact) -> MacroResolutionReason {
    match reason {
        SourceMacroResolutionReasonFact::VisibleDefinition => {
            MacroResolutionReason::VisibleDefinition
        }
        SourceMacroResolutionReasonFact::IncludeGuardIfNDef => {
            MacroResolutionReason::IncludeGuardIfNDef
        }
    }
}

fn map_reference_ranges(
    mapped: &MappedSourcePreprocModel,
    reference: &SourceMacroReferenceFact,
) -> PreprocResult<(MappedPreprocSource, TextRange, TextRange)> {
    let (directive_source, directive_range) =
        map_mapped_source_range(mapped, reference.directive_range)?;
    let (name_source, name_range) = map_mapped_source_range(mapped, reference.name_range)?;
    if directive_source != name_source {
        return Err(PreprocError::MismatchedReferenceRangeFiles {
            event_id: reference.event_id.raw(),
            directive_file_id: directive_source.file_id(),
            name_file_id: name_source.file_id(),
        });
    }
    Ok((directive_source, directive_range, name_range))
}

fn map_include_status(
    mapped: &MappedSourcePreprocModel,
    status: &SourceIncludeStatus,
) -> PreprocResult<IncludeDirectiveStatus> {
    Ok(match status {
        SourceIncludeStatus::Resolved { source } => {
            IncludeDirectiveStatus::Resolved { source: map_mapped_source_id(mapped, *source)? }
        }
        SourceIncludeStatus::Unresolved => IncludeDirectiveStatus::Unresolved,
        SourceIncludeStatus::Unavailable(reason) => {
            IncludeDirectiveStatus::Unavailable(PreprocUnavailable::Source(reason.clone()))
        }
    })
}

fn capability_status(status: &CapabilityStatus) -> PreprocAvailability {
    match status {
        CapabilityStatus::Complete => PreprocAvailability::Complete,
        CapabilityStatus::Partial => PreprocAvailability::Partial,
        CapabilityStatus::Unavailable(reason) => {
            PreprocAvailability::Unavailable(PreprocUnavailable::Source(reason.clone()))
        }
    }
}

fn macro_call_availability(status: &SourceMacroCallStatusFact) -> PreprocAvailability {
    match status {
        SourceMacroCallStatusFact::ExpansionAvailable => PreprocAvailability::Complete,
        SourceMacroCallStatusFact::ExpansionUnavailable(reason) => {
            PreprocAvailability::Unavailable(PreprocUnavailable::Source(reason.clone()))
        }
    }
}

fn macro_expansion_availability(status: &SourceMacroExpansionStatusFact) -> PreprocAvailability {
    match status {
        SourceMacroExpansionStatusFact::Complete => PreprocAvailability::Complete,
        SourceMacroExpansionStatusFact::Unavailable(reason) => {
            PreprocAvailability::Unavailable(PreprocUnavailable::Source(reason.clone()))
        }
    }
}

fn unavailable_error(reason: SourcePreprocUnavailable) -> PreprocError {
    PreprocError::Unavailable { reason: PreprocUnavailable::Source(reason) }
}

fn define_index_for_definition(
    mapped: &MappedSourcePreprocModel,
    definition: &SourceMacroDefinitionFact,
) -> PreprocResult<usize> {
    mapped
        .model
        .defines()
        .iter()
        .position(|define| define.event_id == definition.event_id)
        .ok_or_else(|| {
            PreprocError::SourceQuery(SourcePreprocQueryError::Model(
                SourcePreprocError::MissingEvent { event_id: definition.event_id.raw() },
            ))
        })
}

fn map_definition_ranges(
    mapped: &MappedSourcePreprocModel,
    event_id: u32,
    directive_source_range: SourceRange,
    name_source_range: SourceRange,
) -> PreprocResult<(MappedPreprocSource, TextRange, TextRange)> {
    let (directive_source, directive_range) =
        map_mapped_source_range(mapped, directive_source_range)?;
    let (name_source, name_range) = map_mapped_source_range(mapped, name_source_range)?;
    if directive_source != name_source {
        return Err(PreprocError::MismatchedDefinitionRangeFiles {
            event_id,
            directive_file_id: directive_source.file_id(),
            name_file_id: name_source.file_id(),
        });
    }
    Ok((directive_source, directive_range, name_range))
}

fn map_include_chain(
    mapped: &MappedSourcePreprocModel,
    chain: &[SourceIncludeChainEntry],
) -> PreprocResult<Vec<IncludeChainEntry>> {
    chain
        .iter()
        .map(|entry| {
            let (include_file_id, include_range) = map_source_range(mapped, entry.include_range)?;
            let included_file_id = map_source_id(mapped, entry.included_source)?;
            Ok(IncludeChainEntry {
                include_event_id: entry.include_event_id.raw(),
                include_file_id,
                include_range,
                included_file_id,
            })
        })
        .collect()
}

fn push_unique_macro_reference(refs: &mut Vec<MacroReference>, reference: MacroReference) {
    if refs.iter().any(|existing| {
        existing.file_id == reference.file_id
            && existing.range == reference.range
            && existing.name == reference.name
    }) {
        return;
    }
    refs.push(reference);
}

fn push_unique_macro_reference_context(refs: &mut Vec<MacroReference>, reference: MacroReference) {
    if refs.iter().any(|existing| existing == &reference) {
        return;
    }
    refs.push(reference);
}

fn push_unique_macro_usage_resolution(
    resolutions: &mut Vec<MacroUsageResolution>,
    resolution: MacroUsageResolution,
) {
    if resolutions.iter().any(|existing| existing == &resolution) {
        return;
    }
    resolutions.push(resolution);
}

fn push_unique_macro_expansion_query(
    queries: &mut Vec<MacroExpansionQuery>,
    query: MacroExpansionQuery,
) {
    if queries.iter().any(|existing| existing == &query) {
        return;
    }
    queries.push(query);
}

fn push_unique_recursive_macro_expansion(
    expansions: &mut Vec<RecursiveMacroExpansion>,
    expansion: RecursiveMacroExpansion,
) {
    if expansions.iter().any(|existing| existing == &expansion) {
        return;
    }
    expansions.push(expansion);
}

fn push_unique_macro_expansion_provenance(
    provenances: &mut Vec<MacroExpansionProvenance>,
    provenance: MacroExpansionProvenance,
) {
    if provenances.iter().any(|existing| existing == &provenance) {
        return;
    }
    provenances.push(provenance);
}

fn push_unique_diagnostic_provenance(
    provenances: &mut Vec<DiagnosticProvenance>,
    provenance: DiagnosticProvenance,
) {
    if provenances.iter().any(|existing| existing == &provenance) {
        return;
    }
    provenances.push(provenance);
}

fn push_unique_include_directive(
    directives: &mut Vec<IncludeDirective>,
    directive: IncludeDirective,
) {
    if directives.iter().any(|existing| {
        existing.file_id == directive.file_id
            && existing.range == directive.range
            && existing.target == directive.target
            && existing.status == directive.status
    }) {
        return;
    }
    directives.push(directive);
}

fn push_unique_inactive_branch(branches: &mut Vec<InactiveBranch>, branch: InactiveBranch) {
    if branches
        .iter()
        .any(|existing| existing.file_id == branch.file_id && existing.range == branch.range)
    {
        return;
    }
    branches.push(branch);
}

fn macro_reference_context_capability(references: &[MacroReference]) -> PreprocAvailability {
    if references
        .iter()
        .all(|reference| matches!(reference.capability, PreprocAvailability::Complete))
    {
        return PreprocAvailability::Complete;
    }
    if references
        .iter()
        .any(|reference| matches!(reference.capability, PreprocAvailability::Partial))
    {
        return PreprocAvailability::Partial;
    }
    references
        .iter()
        .find_map(|reference| match &reference.capability {
            PreprocAvailability::Unavailable(reason) => {
                Some(PreprocAvailability::Unavailable(reason.clone()))
            }
            PreprocAvailability::Complete | PreprocAvailability::Partial => None,
        })
        .unwrap_or(PreprocAvailability::Complete)
}

fn push_unique_macro_definition(
    definitions: &mut Vec<MacroDefinition>,
    definition: MacroDefinition,
) {
    if definitions.iter().any(|existing| {
        existing.file_id == definition.file_id
            && existing.name_range == definition.name_range
            && existing.name == definition.name
    }) {
        return;
    }
    definitions.push(definition);
}

fn same_macro_definition(left: &MacroDefinition, right: &MacroDefinition) -> bool {
    left.file_id == right.file_id && left.name_range == right.name_range && left.name == right.name
}

fn same_macro_param_definition(left: &MacroParamDefinition, right: &MacroParamDefinition) -> bool {
    same_macro_definition(&left.macro_definition, &right.macro_definition)
        && left.param_index == right.param_index
        && left.range == right.range
        && left.name == right.name
}

fn push_unique_macro_param_definition(
    definitions: &mut Vec<MacroParamDefinition>,
    definition: MacroParamDefinition,
) {
    if definitions.iter().any(|existing| same_macro_param_definition(existing, &definition)) {
        return;
    }
    definitions.push(definition);
}

fn push_unique_macro_param_reference(
    refs: &mut Vec<MacroParamReference>,
    reference: MacroParamReference,
) {
    if refs.iter().any(|existing| {
        same_macro_definition(&existing.macro_definition, &reference.macro_definition)
            && existing.param_index == reference.param_index
            && existing.file_id == reference.file_id
            && existing.range == reference.range
            && existing.name == reference.name
    }) {
        return;
    }
    refs.push(reference);
}

fn macro_param_reference_context_capability(
    references: &[MacroParamReference],
) -> PreprocAvailability {
    if references
        .iter()
        .any(|reference| matches!(reference.capability, PreprocAvailability::Partial))
    {
        return PreprocAvailability::Partial;
    }
    references
        .iter()
        .find_map(|reference| match &reference.capability {
            PreprocAvailability::Unavailable(reason) => {
                Some(PreprocAvailability::Unavailable(reason.clone()))
            }
            PreprocAvailability::Complete | PreprocAvailability::Partial => None,
        })
        .unwrap_or(PreprocAvailability::Complete)
}

fn preproc_reference_model_file_ids(
    db: &dyn SourceRootDb,
    profile_id: Option<CompilationProfileId>,
) -> Vec<FileId> {
    let plan = db.compilation_plan_for_profile(profile_id);
    let mut file_ids = FxHashSet::default();

    for root in plan.roots.iter().copied() {
        if matches!(
            db.file_kind(root),
            SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader
        ) {
            file_ids.insert(root);
        }
    }
    file_ids.extend(plan.include_only.iter().copied());

    for source_root_id in &plan.source_roots {
        for candidate in db.source_root(*source_root_id).iter() {
            if db.file_is_project_ignored(candidate) {
                continue;
            }
            if matches!(db.file_kind(candidate), SourceFileKind::IncludeHeader) {
                file_ids.insert(candidate);
            }
        }
    }

    for candidate in db.files().iter().copied() {
        if db.file_is_project_ignored(candidate) {
            continue;
        }
        if !matches!(db.file_kind(candidate), SourceFileKind::IncludeHeader) {
            continue;
        }
        let Some(path) = db.file_path(candidate) else {
            continue;
        };
        if plan.include_dirs.iter().any(|include_dir| path.starts_with(include_dir)) {
            file_ids.insert(candidate);
        }
    }

    let mut file_ids = file_ids.into_iter().collect::<Vec<_>>();
    file_ids.sort();
    file_ids
}

fn range_contains_offset(range: TextRange, offset: TextSize) -> bool {
    range.start() <= offset && offset <= range.end()
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use rustc_hash::FxHashSet;
    use triomphe::Arc;
    use utils::{
        get::Get,
        line_index::{TextRange, TextSize},
        paths::{AbsPathBuf, Utf8PathBuf},
    };
    use vfs::{FileId, FileSet, VfsPath, anchored_path::AnchoredPath};

    use super::*;
    use crate::{
        base_db::{
            diagnostics_config::DiagnosticsConfig,
            project::{
                CompilationProfile, CompilationProfileId, Predefine, PredefineSource,
                PreprocessConfig, ProjectConfig,
            },
            salsa::{self, Durability},
            source_db::{
                FileLoader, PreprocVirtualOrigin, SourceDb, SourceDbStorage, SourceFileKind,
                SourceRootDb, SourceRootDbStorage,
            },
            source_root::{SourceRoot, SourceRootId},
        },
        container::InFile,
        db::{HirDb, HirDbStorage, InternDbStorage},
        hir_def::module::ModuleId,
        source_map::IsSrc,
    };

    const TOP: FileId = FileId(0);
    const HEADER: FileId = FileId(1);
    const LEAF: FileId = FileId(2);
    const MANIFEST: FileId = FileId(3);
    const ROOT: SourceRootId = SourceRootId(0);
    const PROFILE: CompilationProfileId = CompilationProfileId(0);

    #[salsa::database(SourceDbStorage, SourceRootDbStorage, InternDbStorage, HirDbStorage)]
    #[derive(Default)]
    struct TestDb {
        storage: salsa::Storage<Self>,
    }

    impl salsa::Database for TestDb {}

    impl fmt::Debug for TestDb {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("TestDb").finish()
        }
    }

    impl FileLoader for TestDb {
        fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId> {
            let source_root_id = SourceRootDb::source_root_id(self, path.anchor_id);
            SourceRootDb::source_root(self, source_root_id).resolve_path(path)
        }
    }

    fn db_with_files(root_text: &str, header_text: &str) -> TestDb {
        db_with_entries(&[(TOP, "rtl/top.v", root_text), (HEADER, "include/defs.vh", header_text)])
    }

    fn db_with_nested_files(root_text: &str, header_text: &str, leaf_text: &str) -> TestDb {
        db_with_entries(&[
            (TOP, "rtl/top.v", root_text),
            (HEADER, "include/defs.vh", header_text),
            (LEAF, "include/leaf.vh", leaf_text),
        ])
    }

    fn db_with_entries(entries: &[(FileId, &str, &str)]) -> TestDb {
        db_with_entries_and_predefines(entries, Vec::new())
    }

    fn db_with_entries_and_predefines(
        entries: &[(FileId, &str, &str)],
        predefines: Vec<String>,
    ) -> TestDb {
        db_with_entries_and_predefine_entries(
            entries,
            predefines.into_iter().map(Predefine::new).collect(),
        )
    }

    fn db_with_entries_and_predefine_entries(
        entries: &[(FileId, &str, &str)],
        predefines: Vec<Predefine>,
    ) -> TestDb {
        let include_dir = abs_path("include");

        let mut file_set = FileSet::default();
        for (file_id, path, _) in entries {
            file_set.insert(*file_id, VfsPath::from(abs_path(path)));
        }
        let root = SourceRoot::new_local_with_source_files(file_set, vec![TOP]);

        let preprocess = PreprocessConfig { predefines, include_dirs: vec![include_dir.clone()] };
        let project_config = ProjectConfig::new(
            vec![Some(PROFILE)],
            vec![CompilationProfile {
                source_roots: vec![ROOT],
                top_modules: Vec::new(),
                preprocess: preprocess.clone(),
            }],
        );

        let mut files = FxHashSet::default();
        for (file_id, _, _) in entries {
            files.insert(*file_id);
        }

        let mut db = TestDb::default();
        db.set_files_with_durability(Box::new(files), Durability::HIGH);
        db.set_project_config_with_durability(Arc::new(project_config), Durability::HIGH);
        db.set_diagnostics_config_with_durability(
            Arc::new(DiagnosticsConfig::default()),
            Durability::HIGH,
        );
        db.set_source_root_with_durability(ROOT, Arc::new(root), Durability::LOW);

        for (file_id, path, text) in entries {
            let path = abs_path(path);
            let vfs_path = VfsPath::from(path.clone());
            db.set_source_root_id_with_durability(*file_id, ROOT, Durability::LOW);
            db.set_file_path_with_durability(*file_id, Some(path), Durability::LOW);
            db.set_file_kind_with_durability(
                *file_id,
                SourceFileKind::from_path(&vfs_path),
                Durability::LOW,
            );
            db.set_file_text_with_durability(*file_id, Arc::from(*text), Durability::LOW);
            db.set_file_preprocess_config_with_durability(
                *file_id,
                Arc::new(preprocess.clone()),
                Durability::LOW,
            );
        }

        db
    }

    fn abs_path(path: &str) -> AbsPathBuf {
        let prefix = if cfg!(windows) { "C:/repo" } else { "/repo" };
        AbsPathBuf::assert(Utf8PathBuf::from(format!("{prefix}/{path}")))
    }

    fn offset(text: &str, needle: &str) -> TextSize {
        TextSize::from(u32::try_from(text.find(needle).unwrap()).unwrap())
    }

    fn offset_after(text: &str, needle: &str) -> TextSize {
        TextSize::from(u32::try_from(text.find(needle).unwrap() + needle.len()).unwrap())
    }

    fn offset_after_n(text: &str, needle: &str, occurrence: usize) -> TextSize {
        let mut cursor = 0;
        for index in 0..=occurrence {
            let relative = text[cursor..].find(needle).unwrap_or_else(|| {
                panic!("missing occurrence {occurrence} of {needle:?} in fixture")
            });
            let absolute = cursor + relative;
            if index == occurrence {
                return TextSize::from(u32::try_from(absolute + needle.len()).unwrap());
            }
            cursor = absolute + needle.len();
        }
        unreachable!()
    }

    fn text_at_range(text: &str, range: TextRange) -> &str {
        &text[usize::from(range.start())..usize::from(range.end())]
    }

    #[test]
    fn preproc_include_usage_resolves_to_header_define() {
        let root_text = r#"`include "defs.vh"
module top;
localparam int W = `HEADER_WIDTH;
endmodule
"#;
        let header_text = "`define HEADER_WIDTH 8\n";
        let db = db_with_files(root_text, header_text);

        let resolution = macro_usage_resolution_at(&db, TOP, offset(root_text, "HEADER_WIDTH"))
            .unwrap()
            .unwrap();
        assert_eq!(resolution.usage.file_id, TOP);
        assert_eq!(resolution.definition.file_id, HEADER);
        assert_eq!(resolution.definition.name.as_str(), "HEADER_WIDTH");
        assert_eq!(text_at_range(header_text, resolution.definition.name_range), "HEADER_WIDTH");

        let include =
            include_directive_at(&db, TOP, offset(root_text, "defs.vh")).unwrap().unwrap();
        assert_eq!(text_at_range(root_text, include.range), "\"defs.vh\"");
        assert!(include_directive_at(&db, TOP, offset(root_text, "`include")).unwrap().is_none());
        let IncludeTarget::Literal { resolved_file, .. } = include.target else {
            panic!("literal include expected");
        };
        assert_eq!(resolved_file, Some(HEADER));
    }

    #[test]
    fn preproc_macro_expansion_queries_map_call_ranges() {
        let root_text = r#"`define OBJ 8
`define LEAF 3
`define WRAP `LEAF
module top;
localparam int A = `OBJ;
localparam int B = `WRAP;
endmodule
"#;
        let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

        let immediate =
            immediate_macro_expansion_at(&db, TOP, offset(root_text, "`OBJ")).unwrap().unwrap();
        let MacroExpansionQuery::Available(immediate) = immediate else {
            panic!("object-like macro expansion should be available");
        };
        assert_eq!(immediate.call.file_id, TOP);
        assert_eq!(text_at_range(root_text, immediate.call.range), "`OBJ");
        assert_eq!(immediate.emitted_token_range.len, 1);
        assert!(matches!(immediate.capability, PreprocAvailability::Complete));

        let recursive =
            recursive_macro_expansion_at(&db, TOP, offset(root_text, "`WRAP")).unwrap().unwrap();
        assert_eq!(recursive.root_call.file_id, TOP);
        assert_eq!(text_at_range(root_text, recursive.root_call.range), "`WRAP");
        assert_eq!(recursive.expansions.len(), 2);
        assert!(recursive.expansions.iter().any(|expansion| !expansion.child_calls.is_empty()));
        assert!(recursive.unavailable.is_empty());
    }

    #[test]
    fn preproc_macro_expansion_materializes_virtual_source_and_token_provenance() {
        let root_text = r#"`define MAKE_DECL(name) logic name;
module top;
`MAKE_DECL(generated)
endmodule
"#;
        let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

        let provenance = macro_expansion_provenance_at(&db, TOP, offset(root_text, "`MAKE_DECL"))
            .unwrap()
            .unwrap();
        let MappedPreprocSource::VirtualFile { path, origin, .. } =
            &provenance.expansion.virtual_source
        else {
            panic!("macro expansion should expose a virtual expansion source");
        };
        assert_eq!(
            path,
            &VfsPath::new_virtual_path("/__vide/preproc/profile-0/expansion/0.sv".to_owned())
        );
        assert_eq!(
            origin,
            &PreprocVirtualOrigin::Expansion { expansion: SourceMacroExpansionId::new(0) }
        );

        let mapped = db.source_preproc_model(TOP);
        let mapped = mapped.as_ref().as_ref().unwrap();
        let virtual_file = mapped.source_map.expansion(SourceMacroExpansionId::new(0)).unwrap();
        assert_eq!(virtual_file.text, "logic generated ;");
        assert_eq!(provenance.expansion.virtual_range, TextRange::new(0.into(), 17.into()));

        let logic = provenance
            .tokens
            .iter()
            .find(|token| token.text.as_str() == "logic")
            .expect("macro body token should be present");
        let TokenProvenance::MacroBody { source, range, .. } = &logic.provenance else {
            panic!("logic should come from the macro body: {logic:?}");
        };
        assert_eq!(source.file_id(), TOP);
        assert_eq!(text_at_range(root_text, *range), "logic");
        assert_eq!(logic.virtual_range, TextRange::new(0.into(), 5.into()));

        let generated = provenance
            .tokens
            .iter()
            .find(|token| token.text.as_str() == "generated")
            .expect("macro argument token should be present");
        let TokenProvenance::MacroArgument { source, range, argument_index, .. } =
            &generated.provenance
        else {
            panic!("generated should come from the macro argument: {generated:?}");
        };
        assert_eq!(*argument_index, 0);
        assert_eq!(source.file_id(), TOP);
        assert_eq!(text_at_range(root_text, *range), "generated");
        assert_eq!(generated.virtual_range, TextRange::new(6.into(), 15.into()));
    }

    #[test]
    fn macro_generated_declaration_hir_range_resolves_to_expanded_token_provenance() {
        let root_text = r#"`define MAKE_DECL(name) logic name;
module top;
`MAKE_DECL(generated)
endmodule
"#;
        let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);
        let (hir_file, _) = db.hir_file_with_source_map(TOP.into());
        let (local_module_id, _) = hir_file.modules.iter().next().unwrap();
        let module_id: ModuleId = InFile::new(TOP.into(), local_module_id);
        let (module, module_src_map) = db.module_with_source_map(module_id);
        let (declaration_id, _) =
            module.declarations.iter().next().expect("generated declaration should lower to HIR");
        let declaration_src = module_src_map
            .get(declaration_id)
            .expect("generated declaration should keep a source-map range");

        let provenance = macro_expansion_provenance_for_range(&db, TOP, declaration_src.range())
            .unwrap()
            .unwrap();

        assert_eq!(provenance.expansion.emitted_token_range.len, 3);
        assert!(
            provenance
                .tokens
                .iter()
                .any(|token| matches!(token.provenance, TokenProvenance::MacroBody { .. }))
        );
        assert!(
            provenance
                .tokens
                .iter()
                .any(|token| matches!(token.provenance, TokenProvenance::MacroArgument { .. }))
        );
    }

    #[test]
    fn diagnostic_provenance_returns_unavailable_for_unsupported_expansion_mapping() {
        let root_text = r#"`define JOIN(a,b) a``b
module top;
wire `JOIN(foo,bar);
endmodule
"#;
        let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);
        let call_range =
            TextRange::new(offset(root_text, "`JOIN"), offset_after(root_text, "`JOIN(foo,bar)"));

        let provenance = diagnostic_provenance_for_range(&db, TOP, call_range).unwrap().unwrap();
        assert!(matches!(
            provenance,
            DiagnosticProvenance::Unavailable(PreprocUnavailable::Source(_))
        ));
    }

    #[test]
    fn preproc_nested_include_chain_maps_to_file_ids() {
        let root_text = r#"`include "defs.vh"
module top;
localparam int W = `LEAF_WIDTH;
endmodule
"#;
        let header_text = "`include \"leaf.vh\"\n";
        let leaf_text = "`define LEAF_WIDTH 4\n";
        let db = db_with_nested_files(root_text, header_text, leaf_text);

        let resolution =
            macro_usage_resolution_at(&db, TOP, offset(root_text, "LEAF_WIDTH")).unwrap().unwrap();

        assert_eq!(resolution.definition.file_id, LEAF);
        assert_eq!(resolution.definition_provenance.file_id, LEAF);
        assert_eq!(resolution.include_chain.len(), 2);
        assert_eq!(resolution.include_chain[0].include_file_id, TOP);
        assert_eq!(resolution.include_chain[0].included_file_id, HEADER);
        assert!(
            text_at_range(root_text, resolution.include_chain[0].include_range).contains("defs.vh")
        );
        assert_eq!(resolution.include_chain[1].include_file_id, HEADER);
        assert_eq!(resolution.include_chain[1].included_file_id, LEAF);
        assert!(
            text_at_range(header_text, resolution.include_chain[1].include_range)
                .contains("leaf.vh")
        );
    }

    #[test]
    fn preproc_unsaved_include_buffer_updates_query_result() {
        let root_text = r#"`include "defs.vh"
module top;
localparam int W = `HEADER_WIDTH;
endmodule
"#;
        let mut db = db_with_files(root_text, "`define OTHER_WIDTH 8\n");

        assert!(
            macro_usage_resolution_at(&db, TOP, offset(root_text, "HEADER_WIDTH"))
                .unwrap()
                .is_none()
        );

        db.set_file_text_with_durability(
            HEADER,
            Arc::from("`define HEADER_WIDTH 16\n"),
            Durability::LOW,
        );

        let resolution = macro_usage_resolution_at(&db, TOP, offset(root_text, "HEADER_WIDTH"))
            .unwrap()
            .unwrap();
        assert_eq!(resolution.definition.file_id, HEADER);
        assert_eq!(resolution.definition.name.as_str(), "HEADER_WIDTH");
    }

    #[test]
    fn preproc_visible_macro_names_include_predefines_without_file_mapping() {
        let root_text = r#"`define A005_LOCAL 1
module top;
localparam int W = `A005_;
endmodule
"#;
        let db = db_with_entries_and_predefines(
            &[(TOP, "rtl/top.v", root_text)],
            vec!["A005_MAGIC=42".to_owned()],
        );

        let names = visible_macro_names_at(&db, TOP, offset_after(root_text, "`A005_")).unwrap();

        assert!(names.iter().any(|name| name == "A005_LOCAL"), "{names:?}");
        assert!(names.iter().any(|name| name == "A005_MAGIC"), "{names:?}");
    }

    #[test]
    fn preproc_manifest_predefine_definition_uses_manifest_provenance() {
        let root_text = r#"`ifdef FROM_MANIFEST
module top;
localparam int W = `FROM_MANIFEST;
endmodule
`endif
"#;
        let manifest_text = "defines = [\"FROM_MANIFEST=1\"]\n";
        let manifest_range = TextRange::new(
            offset(manifest_text, "\"FROM_MANIFEST=1\""),
            offset_after(manifest_text, "\"FROM_MANIFEST=1\""),
        );
        let predefine = Predefine::with_source(
            "FROM_MANIFEST=1",
            PredefineSource { path: abs_path("vide.toml"), range: manifest_range },
        );
        let db = db_with_entries_and_predefine_entries(
            &[(TOP, "rtl/top.v", root_text), (MANIFEST, "vide.toml", manifest_text)],
            vec![predefine],
        );

        let resolution = macro_reference_definitions_at(
            &db,
            TOP,
            offset_after_n(root_text, "`FROM_MANIFEST", 0),
        )
        .unwrap()
        .unwrap();
        assert!(
            resolution.definitions.iter().any(|definition| {
                definition.file_id == MANIFEST && definition.name_range == manifest_range
            }),
            "predefine reference should target the manifest source range: {resolution:?}"
        );

        let definition =
            macro_definition_at(&db, MANIFEST, manifest_range.start()).unwrap().unwrap();
        assert_eq!(definition.file_id, MANIFEST);
        assert_eq!(definition.name.as_str(), "FROM_MANIFEST");
        assert_eq!(definition.name_range, manifest_range);
        assert_eq!(text_at_range(manifest_text, definition.name_range), "\"FROM_MANIFEST=1\"");

        let references = macro_references(&db, MANIFEST, &definition).unwrap();
        assert!(
            references.references.iter().any(|reference| {
                reference.file_id == TOP
                    && text_at_range(root_text, reference.range) == "FROM_MANIFEST"
            }),
            "manifest predefine definition should find source references: {references:?}"
        );
    }

    #[test]
    fn preproc_visible_macro_names_follow_define_undef_boundaries() {
        let root_text = r#"`define A005_LOCAL 1
`undef A005_LOCAL
`define A005_NEXT 2
module top;
localparam int W = `A005_;
endmodule
"#;
        let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

        let names_after_define =
            visible_macro_names_at(&db, TOP, offset_after(root_text, "`define A005_LOCAL 1\n"))
                .unwrap();
        let names_after_undef =
            visible_macro_names_at(&db, TOP, offset_after(root_text, "`undef A005_LOCAL\n"))
                .unwrap();
        let names_after_next =
            visible_macro_names_at(&db, TOP, offset_after(root_text, "`define A005_NEXT 2\n"))
                .unwrap();

        assert!(names_after_define.iter().any(|name| name == "A005_LOCAL"));
        assert!(!names_after_undef.iter().any(|name| name == "A005_LOCAL"));
        assert!(names_after_next.iter().any(|name| name == "A005_NEXT"));
    }

    #[test]
    fn preproc_inactive_branch_uses_header_define() {
        let root_text = r#"`include "defs.vh"
`ifndef HEADER_FLAG
wire disabled_by_header;
`endif
wire active;
"#;
        let header_text = "`define HEADER_FLAG\n";
        let db = db_with_files(root_text, header_text);

        let branches = inactive_branches(&db, TOP).unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].file_id, TOP);
        assert!(text_at_range(root_text, branches[0].range).contains("disabled_by_header"));
    }

    #[test]
    fn preproc_included_define_references_include_root_conditionals() {
        let root_text = r#"`include "defs.vh"
`ifdef HEADER_FLAG
localparam int ENABLED = `HEADER_FLAG;
`endif
"#;
        let header_text = "`define HEADER_FLAG 1\n";
        let db = db_with_files(root_text, header_text);
        let definition = macro_definition_at(&db, HEADER, offset_after(header_text, "`define "))
            .unwrap()
            .unwrap();

        assert_eq!(definition.source.file_id(), HEADER);
        assert!(matches!(definition.capability, PreprocAvailability::Complete));

        let refs = macro_references(&db, HEADER, &definition).unwrap().references;

        assert!(refs.iter().any(|reference| {
            reference.file_id == TOP && text_at_range(root_text, reference.range) == "HEADER_FLAG"
        }));
        assert!(refs.iter().any(|reference| {
            reference.file_id == TOP
                && matches!(
                    reference.resolution,
                    MacroResolution::Resolved {
                        reason: MacroResolutionReason::VisibleDefinition,
                        ..
                    }
                )
                && text_at_range(root_text, reference.range) == "HEADER_FLAG"
        }));

        let definitions =
            macro_reference_definitions_at(&db, TOP, offset_after(root_text, "ENABLED = `"))
                .unwrap()
                .unwrap();
        assert_eq!(text_at_range(root_text, definitions.range), "`HEADER_FLAG");
        assert!(matches!(definitions.capability, PreprocAvailability::Complete));
        assert!(definitions.definitions.iter().any(|indexed| {
            indexed.file_id == HEADER
                && indexed.name_range == definition.name_range
                && indexed.name == definition.name
        }));
    }

    #[test]
    fn preproc_header_ifdef_reference_uses_including_root_context() {
        let root_text = r#"`include "defs.vh"
`include "leaf.vh"
"#;
        let header_text = "`define FEATURE_B 1\n";
        let leaf_text = r#"`ifdef FEATURE_B
wire enabled;
`endif
"#;
        let db = db_with_nested_files(root_text, header_text, leaf_text);

        let definitions = macro_reference_definitions_at(&db, LEAF, offset(leaf_text, "FEATURE_B"))
            .unwrap()
            .unwrap();

        assert_eq!(text_at_range(leaf_text, definitions.range), "FEATURE_B");
        assert!(definitions.definitions.iter().any(|definition| {
            definition.file_id == HEADER
                && text_at_range(header_text, definition.name_range) == "FEATURE_B"
        }));
    }

    #[test]
    fn preproc_header_macro_body_references_use_expansion_context() {
        let root_text = r#"`include "defs.vh"
module top;
localparam int W = `DEMO_WIDTH;
localparam int N = `DEMO_NEXT(1);
localparam int R = `DEMO_RESET;
endmodule
"#;
        let header_text = r#"`ifndef SHARED_DEFS_SVH
`define SHARED_DEFS_SVH
`include "leaf.vh"
`define DEMO_WIDTH `MATH_WIDTH
`define DEMO_RESET {`DEMO_WIDTH{1'b0}}
`define DEMO_NEXT(value) ((value) + `MATH_ONE)
`endif
"#;
        let leaf_text = r#"`define MATH_WIDTH 12
`define MATH_ONE 12'd1
"#;
        let db = db_with_nested_files(root_text, header_text, leaf_text);

        let math_width =
            macro_reference_definitions_at(&db, HEADER, offset(header_text, "MATH_WIDTH"))
                .unwrap()
                .unwrap();
        assert!(math_width.definitions.iter().any(|definition| {
            definition.file_id == LEAF
                && text_at_range(leaf_text, definition.name_range) == "MATH_WIDTH"
        }));

        let math_one = macro_reference_definitions_at(&db, HEADER, offset(header_text, "MATH_ONE"))
            .unwrap()
            .unwrap();
        assert!(math_one.definitions.iter().any(|definition| {
            definition.file_id == LEAF
                && text_at_range(leaf_text, definition.name_range) == "MATH_ONE"
        }));

        let demo_width = macro_reference_definitions_at(
            &db,
            HEADER,
            offset_after(header_text, "`define DEMO_RESET {`"),
        )
        .unwrap()
        .unwrap();
        assert!(demo_width.definitions.iter().any(|definition| {
            definition.file_id == HEADER
                && text_at_range(header_text, definition.name_range) == "DEMO_WIDTH"
        }));
    }

    #[test]
    fn preproc_macro_param_references_resolve_to_formals() {
        let root_text = r#"`include "defs.vh"
module top;
localparam int W = `SHIFT(4, 1);
endmodule
"#;
        let header_text = "`define SHIFT(value, amount) ((value) << amount)\n";
        let db = db_with_files(root_text, header_text);

        let value_definition =
            macro_param_definition_at(&db, HEADER, offset_after(header_text, "SHIFT("))
                .unwrap()
                .unwrap();
        assert_eq!(value_definition.name.as_str(), "value");
        assert_eq!(text_at_range(header_text, value_definition.range), "value");

        let value_reference = macro_param_reference_definitions_at(
            &db,
            HEADER,
            offset_after(header_text, "SHIFT(value, amount) (("),
        )
        .unwrap()
        .unwrap();
        assert_eq!(text_at_range(header_text, value_reference.range), "value");
        assert!(value_reference.definitions.iter().any(|definition| {
            definition.param_index == value_definition.param_index
                && text_at_range(header_text, definition.range) == "value"
        }));

        let refs = macro_param_references(&db, HEADER, &value_definition).unwrap().references;
        assert!(refs.iter().any(|reference| {
            reference.file_id == HEADER && text_at_range(header_text, reference.range) == "value"
        }));
        assert!(
            !refs.iter().any(|reference| text_at_range(header_text, reference.range) == "amount")
        );
    }

    #[test]
    fn preproc_header_reference_reports_all_including_context_definitions() {
        let root_text = r#"`define WIDTH 8
`include "defs.vh"
`undef WIDTH
`define WIDTH 16
`include "defs.vh"
"#;
        let header_text = "localparam int W = `WIDTH;\n";
        let db = db_with_files(root_text, header_text);

        let definitions = macro_reference_definitions_at(&db, HEADER, offset(header_text, "WIDTH"))
            .unwrap()
            .unwrap();

        assert_eq!(text_at_range(header_text, definitions.range), "`WIDTH");
        assert_eq!(definitions.definitions.len(), 2);
        assert!(definitions.definitions.iter().any(|definition| {
            definition.file_id == TOP
                && definition.name_range.start() == offset_after_n(root_text, "`define ", 0)
        }));
        assert!(definitions.definitions.iter().any(|definition| {
            definition.file_id == TOP
                && definition.name_range.start() == offset_after_n(root_text, "`define ", 1)
        }));
    }

    #[test]
    fn preproc_header_macro_body_reference_reports_all_expansion_context_definitions() {
        let root_text = r#"`define WIDTH 8
`include "defs.vh"
localparam int A = `USE_WIDTH;
`undef WIDTH
`define WIDTH 16
`include "defs.vh"
localparam int B = `USE_WIDTH;
"#;
        let header_text = "`define USE_WIDTH `WIDTH\n";
        let db = db_with_files(root_text, header_text);

        let definitions =
            macro_reference_definitions_at(&db, HEADER, offset_after(header_text, "USE_WIDTH `"))
                .unwrap()
                .unwrap();

        assert_eq!(text_at_range(header_text, definitions.range), "`WIDTH");
        assert_eq!(definitions.definitions.len(), 2);
        assert!(definitions.definitions.iter().any(|definition| {
            definition.file_id == TOP
                && definition.name_range.start() == offset_after_n(root_text, "`define ", 0)
        }));
        assert!(definitions.definitions.iter().any(|definition| {
            definition.file_id == TOP
                && definition.name_range.start() == offset_after_n(root_text, "`define ", 1)
        }));
    }

    #[test]
    fn preproc_macro_definition_at_only_hits_name_range() {
        let root_text = "`define HEADER_FLAG 1\n";
        let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

        assert!(macro_definition_at(&db, TOP, offset(root_text, "`define")).unwrap().is_none());

        let definition =
            macro_definition_at(&db, TOP, offset(root_text, "HEADER_FLAG")).unwrap().unwrap();
        assert_eq!(text_at_range(root_text, definition.name_range), "HEADER_FLAG");
        assert_ne!(definition.directive_range, definition.name_range);
    }

    #[test]
    fn preproc_ifndef_guard_reference_resolves_to_following_define() {
        let root_text = "`include \"defs.vh\"\n";
        let header_text = r#"`ifndef HEADER_FLAG
`define HEADER_FLAG
`endif
"#;
        let db = db_with_files(root_text, header_text);
        let resolution =
            macro_reference_definitions_at(&db, HEADER, offset(header_text, "HEADER_FLAG"))
                .unwrap()
                .unwrap();

        assert!(resolution.references.iter().any(|reference| reference.file_id == HEADER));
        let definition =
            resolution.definitions.iter().find(|definition| definition.file_id == HEADER).unwrap();
        assert_eq!(text_at_range(header_text, definition.name_range), "HEADER_FLAG");

        let refs = macro_references(&db, HEADER, definition).unwrap().references;
        assert!(refs.iter().any(|reference| {
            reference.file_id == HEADER
                && reference.range.start() == offset(header_text, "HEADER_FLAG")
                && text_at_range(header_text, reference.range) == "HEADER_FLAG"
        }));
    }

    #[test]
    fn preproc_project_header_guard_reference_is_indexed_without_include() {
        let root_text = "module top; endmodule\n";
        let header_text = r#"`ifndef HEADER_FLAG
`define HEADER_FLAG
`endif
"#;
        let db = db_with_files(root_text, header_text);
        let resolution =
            macro_reference_definitions_at(&db, HEADER, offset(header_text, "HEADER_FLAG"))
                .unwrap()
                .unwrap();

        assert!(resolution.references.iter().any(|reference| reference.file_id == HEADER));
        assert!(resolution.definitions.iter().any(|definition| {
            definition.file_id == HEADER
                && text_at_range(header_text, definition.name_range) == "HEADER_FLAG"
        }));
    }
}
