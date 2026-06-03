use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use crate::{
    directive_index::{
        MacroDefine, MacroDirective, MacroDirectiveKind, MacroIncludeTarget, MacroUsage,
        PreprocFileIndex,
    },
    trace::{
        ExpandedTokenId, ExpandedTokenOrigin, MacroExpansionEvent, PREPROC_TRACE_CAPABILITY,
        PreprocTraceResult, TraceCapability,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacroProfileId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacroDefId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacroUseId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MacroName(pub SmolStr);

impl MacroName {
    pub fn new(name: impl Into<SmolStr>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PredefineSource {
    ProjectConfig { file_id: FileId, range: TextRange },
    CommandLine,
    Toolchain,
    Builtin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceOrigin {
    File { file_id: FileId, range: TextRange },
    VirtualPredefine { profile: MacroProfileId, name: MacroName, source: PredefineSource },
    Unsupported { reason: SmolStr },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroPredefine {
    pub name: MacroName,
    pub value: Option<SmolStr>,
    pub source: PredefineSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMacroInput {
    pub file_id: FileId,
    pub index: PreprocFileIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralIncludeInput {
    pub from_file: FileId,
    pub include_index: usize,
    pub to_file: FileId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroDbInput {
    pub profile: MacroProfileId,
    pub roots: Vec<FileId>,
    pub files: Vec<FileMacroInput>,
    pub predefines: Vec<MacroPredefine>,
    pub literal_includes: Vec<LiteralIncludeInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroSource {
    pub id: MacroDefId,
    pub name: MacroName,
    pub origin: SourceOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroQueryFailure {
    CapabilityUnavailable { capability: SmolStr, reason: SmolStr },
    InactiveBranch { file_id: FileId, range: TextRange },
    UnknownDefinition { id: MacroDefId },
    Unresolved { name: MacroName },
    Unsupported { reason: SmolStr },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroUseResolution {
    Resolved(MacroDefId),
    Unresolved,
    Failed(MacroQueryFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroUse {
    pub id: MacroUseId,
    pub file_id: FileId,
    pub name: MacroName,
    pub range: Option<TextRange>,
    pub resolution: MacroUseResolution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroReference {
    pub use_id: MacroUseId,
    pub file_id: FileId,
    pub range: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroDefinitionAtResult {
    Definition(MacroSource),
    NoMacroUse,
    Failed(MacroQueryFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroReferencesResult {
    References(Vec<MacroReference>),
    Failed(MacroQueryFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncludeTargetAtResult {
    Target(FileId),
    NoInclude,
    Failed(MacroQueryFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroDb {
    profile: MacroProfileId,
    roots: Vec<FileId>,
    files: Vec<FileMacroInput>,
    predefines: Vec<MacroPredefine>,
    literal_includes: Vec<LiteralIncludeInput>,
    definitions: Vec<MacroSource>,
    uses: Vec<MacroUse>,
    env_snapshots: Vec<EnvSnapshot>,
    replay_barriers: Vec<ReplayBarrier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EnvSnapshot {
    file_id: FileId,
    offset: TextSize,
    visible: Vec<MacroDefId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReplayBarrier {
    file_id: FileId,
    offset: TextSize,
    failure: MacroQueryFailure,
}

impl MacroDb {
    pub fn new(input: MacroDbInput) -> Self {
        let MacroDbInput { profile, roots, files, predefines, literal_includes } = input;
        let roots =
            if roots.is_empty() { files.iter().map(|file| file.file_id).collect() } else { roots };
        let mut definitions = Vec::new();
        let mut uses = Vec::new();
        let mut env_snapshots = Vec::new();
        let mut replay_barriers = Vec::new();
        let mut predefine_env = FxHashMap::default();

        for predefine in &predefines {
            let id = MacroDefId(definitions.len() as u32);
            let source = MacroSource {
                id,
                name: predefine.name.clone(),
                origin: SourceOrigin::VirtualPredefine {
                    profile,
                    name: predefine.name.clone(),
                    source: predefine.source.clone(),
                },
            };
            predefine_env.insert(source.name.clone(), id);
            definitions.push(source);
        }

        let mut replay = ReplayState {
            files: &files,
            file_indices: files
                .iter()
                .enumerate()
                .map(|(index, file)| (file.file_id, index))
                .collect(),
            include_edges: literal_includes
                .iter()
                .map(|include| ((include.from_file, include.include_index), include.to_file))
                .collect(),
            definitions: &mut definitions,
            uses: &mut uses,
            env_snapshots: &mut env_snapshots,
            replay_barriers: &mut replay_barriers,
        };

        for root in &roots {
            let mut env = predefine_env.clone();
            let mut include_stack = Vec::new();
            if let Some(file_index) = replay.file_indices.get(root).copied() {
                replay.replay_file(file_index, &mut env, &mut include_stack);
            } else {
                replay.replay_barriers.push(ReplayBarrier {
                    file_id: *root,
                    offset: TextSize::from(0),
                    failure: MacroQueryFailure::CapabilityUnavailable {
                        capability: SmolStr::new("macrodb_root_replay"),
                        reason: SmolStr::new("root file is not part of MacroDb input"),
                    },
                });
            }
        }

        Self {
            profile,
            roots,
            files,
            predefines,
            literal_includes,
            definitions,
            uses,
            env_snapshots,
            replay_barriers,
        }
    }

    pub fn profile(&self) -> MacroProfileId {
        self.profile
    }

    pub fn roots(&self) -> &[FileId] {
        &self.roots
    }

    pub fn files(&self) -> &[FileMacroInput] {
        &self.files
    }

    pub fn predefines(&self) -> &[MacroPredefine] {
        &self.predefines
    }

    pub fn literal_includes(&self) -> &[LiteralIncludeInput] {
        &self.literal_includes
    }

    pub fn definitions(&self) -> &[MacroSource] {
        &self.definitions
    }

    pub fn definition(&self, id: MacroDefId) -> Option<&MacroSource> {
        self.definitions.get(id.0 as usize)
    }

    pub fn macro_uses(&self) -> &[MacroUse] {
        &self.uses
    }

    pub fn macro_use(&self, id: MacroUseId) -> Option<&MacroUse> {
        self.uses.get(id.0 as usize)
    }

    pub fn macro_use_at(&self, file_id: FileId, offset: TextSize) -> Option<&MacroUse> {
        self.uses.iter().find(|macro_use| {
            macro_use.file_id == file_id && range_contains(macro_use.range, offset)
        })
    }

    pub fn macro_env_at(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Result<Vec<MacroSource>, MacroQueryFailure> {
        if let Some(failure) = self.replay_failure_before(file_id, offset) {
            return Err(failure);
        }

        let snapshot = self
            .env_snapshots
            .iter()
            .filter(|snapshot| snapshot.file_id == file_id && snapshot.offset <= offset)
            .max_by_key(|snapshot| snapshot.offset)
            .ok_or_else(|| MacroQueryFailure::CapabilityUnavailable {
                capability: SmolStr::new("macro_env_at"),
                reason: SmolStr::new("file is not part of this MacroDb input"),
            })?;
        Ok(snapshot.visible.iter().filter_map(|id| self.definition(*id).cloned()).collect())
    }

    pub fn macro_definition_at(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> MacroDefinitionAtResult {
        if let Some(range) = self.inactive_range_at(file_id, offset) {
            return MacroDefinitionAtResult::Failed(MacroQueryFailure::InactiveBranch {
                file_id,
                range,
            });
        }
        if let Some(failure) = self.replay_failure_before(file_id, offset) {
            return MacroDefinitionAtResult::Failed(failure);
        }

        let Some(macro_use) = self.macro_use_at(file_id, offset) else {
            return MacroDefinitionAtResult::NoMacroUse;
        };

        match &macro_use.resolution {
            MacroUseResolution::Resolved(id) => self
                .definition(*id)
                .cloned()
                .map(MacroDefinitionAtResult::Definition)
                .unwrap_or_else(|| {
                    MacroDefinitionAtResult::Failed(MacroQueryFailure::UnknownDefinition {
                        id: *id,
                    })
                }),
            MacroUseResolution::Unresolved => {
                MacroDefinitionAtResult::Failed(MacroQueryFailure::Unresolved {
                    name: macro_use.name.clone(),
                })
            }
            MacroUseResolution::Failed(failure) => MacroDefinitionAtResult::Failed(failure.clone()),
        }
    }

    pub fn macro_references(&self, definition: MacroDefId) -> MacroReferencesResult {
        if let Some(failure) = self.first_replay_failure() {
            return MacroReferencesResult::Failed(failure);
        }
        if self.definition(definition).is_none() {
            return MacroReferencesResult::Failed(MacroQueryFailure::UnknownDefinition {
                id: definition,
            });
        }

        MacroReferencesResult::References(
            self.uses
                .iter()
                .filter_map(|macro_use| match macro_use.resolution {
                    MacroUseResolution::Resolved(id) if id == definition => Some(MacroReference {
                        use_id: macro_use.id,
                        file_id: macro_use.file_id,
                        range: macro_use.range,
                    }),
                    _ => None,
                })
                .collect(),
        )
    }

    pub fn macro_references_at_definition(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> MacroReferencesResult {
        if let Some(range) = self.inactive_range_at(file_id, offset) {
            return MacroReferencesResult::Failed(MacroQueryFailure::InactiveBranch {
                file_id,
                range,
            });
        }
        if let Some(failure) = self.replay_failure_before(file_id, offset) {
            return MacroReferencesResult::Failed(failure);
        }

        let Some(definition) = self.definition_at_source(file_id, offset) else {
            return MacroReferencesResult::Failed(MacroQueryFailure::CapabilityUnavailable {
                capability: SmolStr::new("macro_references_at_definition"),
                reason: SmolStr::new("offset does not point at a MacroDb definition source"),
            });
        };

        self.macro_references(definition)
    }

    pub fn trace_capability(&self) -> TraceCapability {
        TraceCapability::missing_preproc_trace()
    }

    pub fn expansion_for_use(
        &self,
        _use_id: MacroUseId,
    ) -> PreprocTraceResult<&MacroExpansionEvent> {
        PreprocTraceResult::missing_preproc_trace()
    }

    pub fn origin_for_expanded_token(
        &self,
        _token_id: ExpandedTokenId,
    ) -> PreprocTraceResult<ExpandedTokenOrigin> {
        PreprocTraceResult::missing_preproc_trace()
    }

    pub fn include_target_at(&self, file_id: FileId, offset: TextSize) -> IncludeTargetAtResult {
        let Some((include_index, target)) = self.include_at_source(file_id, offset) else {
            return IncludeTargetAtResult::NoInclude;
        };

        match target {
            MacroIncludeTarget::Literal { .. } => self
                .literal_includes
                .iter()
                .find(|include| {
                    include.from_file == file_id && include.include_index == include_index
                })
                .map(|include| IncludeTargetAtResult::Target(include.to_file))
                .unwrap_or_else(|| {
                    IncludeTargetAtResult::Failed(MacroQueryFailure::CapabilityUnavailable {
                        capability: SmolStr::new("literal_include_replay"),
                        reason: SmolStr::new("literal include edge was not supplied"),
                    })
                }),
            MacroIncludeTarget::Token { .. } => {
                IncludeTargetAtResult::Failed(missing_preproc_trace_failure())
            }
        }
    }

    fn definition_at_source(&self, file_id: FileId, offset: TextSize) -> Option<MacroDefId> {
        self.definitions.iter().find_map(|definition| match definition.origin {
            SourceOrigin::File { file_id: origin_file, range }
                if origin_file == file_id && range_contains(Some(range), offset) =>
            {
                Some(definition.id)
            }
            _ => None,
        })
    }

    fn inactive_range_at(&self, file_id: FileId, offset: TextSize) -> Option<TextRange> {
        self.files
            .iter()
            .find(|file| file.file_id == file_id)?
            .index
            .inactive_ranges
            .iter()
            .copied()
            .find(|range| range_contains(Some(*range), offset))
    }

    fn include_at_source(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Option<(usize, MacroIncludeTarget)> {
        let file = self.files.iter().find(|file| file.file_id == file_id)?;
        file.index.directives.iter().find_map(|directive| {
            if directive.kind != MacroDirectiveKind::Include
                || !range_contains(directive.range, offset)
            {
                return None;
            }
            let include = file.index.includes.get(directive.index)?;
            Some((directive.index, include.target.clone()))
        })
    }

    fn replay_failure_before(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Option<MacroQueryFailure> {
        self.replay_barriers
            .iter()
            .find(|barrier| barrier.file_id == file_id && barrier.offset <= offset)
            .map(|barrier| barrier.failure.clone())
    }

    fn first_replay_failure(&self) -> Option<MacroQueryFailure> {
        self.replay_barriers.first().map(|barrier| barrier.failure.clone())
    }
}

struct ReplayState<'a> {
    files: &'a [FileMacroInput],
    file_indices: FxHashMap<FileId, usize>,
    include_edges: FxHashMap<(FileId, usize), FileId>,
    definitions: &'a mut Vec<MacroSource>,
    uses: &'a mut Vec<MacroUse>,
    env_snapshots: &'a mut Vec<EnvSnapshot>,
    replay_barriers: &'a mut Vec<ReplayBarrier>,
}

impl ReplayState<'_> {
    fn replay_file(
        &mut self,
        file_index: usize,
        env: &mut FxHashMap<MacroName, MacroDefId>,
        include_stack: &mut Vec<FileId>,
    ) {
        let file_id = self.files[file_index].file_id;
        include_stack.push(file_id);
        push_env_snapshot(file_id, TextSize::from(0), env, self.env_snapshots);

        let directives = self.files[file_index].index.directives.clone();
        for directive in directives {
            match directive.kind {
                MacroDirectiveKind::Define => {
                    let define = self.files[file_index].index.defines.get(directive.index).cloned();
                    if let Some(source) = define.as_ref().and_then(|define| {
                        macro_source_from_define(file_id, define, self.definitions.len())
                    }) {
                        env.insert(source.name.clone(), source.id);
                        self.definitions.push(source);
                    }
                }
                MacroDirectiveKind::Undef => {
                    if let Some(undef) = self.files[file_index].index.undefs.get(directive.index)
                        && let Some(name) = &undef.name
                    {
                        env.remove(&MacroName::new(name.clone()));
                    }
                }
                MacroDirectiveKind::Usage => {
                    let usage = self.files[file_index].index.usages.get(directive.index).cloned();
                    if let Some(macro_use) = usage.as_ref().and_then(|usage| {
                        macro_use_from_usage(file_id, usage, env, self.uses.len())
                    }) {
                        self.uses.push(macro_use);
                    }
                }
                MacroDirectiveKind::Include => {
                    self.replay_include(file_index, &directive, env, include_stack);
                }
                MacroDirectiveKind::Conditional | MacroDirectiveKind::Branch => {}
            }
            push_env_snapshot(file_id, directive_offset(&directive), env, self.env_snapshots);
        }

        include_stack.pop();
    }

    fn replay_include(
        &mut self,
        file_index: usize,
        directive: &MacroDirective,
        env: &mut FxHashMap<MacroName, MacroDefId>,
        include_stack: &mut Vec<FileId>,
    ) {
        let file_id = self.files[file_index].file_id;
        let offset = directive_offset(directive);
        let Some(include) = self.files[file_index].index.includes.get(directive.index) else {
            self.record_barrier(
                file_id,
                offset,
                MacroQueryFailure::CapabilityUnavailable {
                    capability: SmolStr::new("literal_include_replay"),
                    reason: SmolStr::new("include directive event is missing"),
                },
            );
            return;
        };

        match &include.target {
            MacroIncludeTarget::Literal { .. } => {
                let Some(included_file_id) =
                    self.include_edges.get(&(file_id, directive.index)).copied()
                else {
                    self.record_barrier(
                        file_id,
                        offset,
                        MacroQueryFailure::CapabilityUnavailable {
                            capability: SmolStr::new("literal_include_replay"),
                            reason: SmolStr::new("literal include edge was not supplied"),
                        },
                    );
                    return;
                };
                if include_stack.contains(&included_file_id) {
                    self.record_barrier(
                        file_id,
                        offset,
                        MacroQueryFailure::Unsupported {
                            reason: SmolStr::new("literal include cycle"),
                        },
                    );
                    return;
                }
                let Some(included_index) = self.file_indices.get(&included_file_id).copied() else {
                    self.record_barrier(
                        file_id,
                        offset,
                        MacroQueryFailure::CapabilityUnavailable {
                            capability: SmolStr::new("literal_include_replay"),
                            reason: SmolStr::new("included file is not part of MacroDb input"),
                        },
                    );
                    return;
                };
                let barrier_start = self.replay_barriers.len();
                self.replay_file(included_index, env, include_stack);
                if let Some(barrier) = self.replay_barriers.get(barrier_start) {
                    self.record_barrier(file_id, offset, barrier.failure.clone());
                }
            }
            MacroIncludeTarget::Token { .. } => {
                self.record_barrier(file_id, offset, missing_preproc_trace_failure());
            }
        }
    }

    fn record_barrier(&mut self, file_id: FileId, offset: TextSize, failure: MacroQueryFailure) {
        self.replay_barriers.push(ReplayBarrier { file_id, offset, failure });
    }
}

fn macro_source_from_define(
    file_id: FileId,
    define: &MacroDefine,
    next_id: usize,
) -> Option<MacroSource> {
    let name = MacroName::new(define.name.clone()?);
    let origin = define
        .name_range
        .or(define.range)
        .map(|range| SourceOrigin::File { file_id, range })
        .unwrap_or_else(|| SourceOrigin::Unsupported {
            reason: SmolStr::new("macro define has no source range"),
        });
    Some(MacroSource { id: MacroDefId(next_id as u32), name, origin })
}

fn macro_use_from_usage(
    file_id: FileId,
    usage: &MacroUsage,
    env: &FxHashMap<MacroName, MacroDefId>,
    next_id: usize,
) -> Option<MacroUse> {
    let name = MacroName::new(usage.name.clone()?);
    let resolution = env
        .get(&name)
        .copied()
        .map(MacroUseResolution::Resolved)
        .unwrap_or(MacroUseResolution::Unresolved);
    Some(MacroUse { id: MacroUseId(next_id as u32), file_id, name, range: usage.range, resolution })
}

fn push_env_snapshot(
    file_id: FileId,
    offset: TextSize,
    env: &FxHashMap<MacroName, MacroDefId>,
    env_snapshots: &mut Vec<EnvSnapshot>,
) {
    let mut visible = env.values().copied().collect::<Vec<_>>();
    visible.sort_by_key(|id| id.0);
    env_snapshots.push(EnvSnapshot { file_id, offset, visible });
}

fn directive_offset(directive: &MacroDirective) -> TextSize {
    directive.range.map(|range| range.end()).unwrap_or_else(|| TextSize::from(0))
}

fn missing_preproc_trace_failure() -> MacroQueryFailure {
    MacroQueryFailure::CapabilityUnavailable {
        capability: SmolStr::new(PREPROC_TRACE_CAPABILITY),
        reason: SmolStr::new("MissingPreprocTrace"),
    }
}

fn range_contains(range: Option<TextRange>, offset: TextSize) -> bool {
    range.is_some_and(|range| range.contains(offset))
}

#[cfg(test)]
mod tests {
    use syntax::SyntaxTreeOptions;

    use super::*;
    use crate::{
        directive_index::preproc_file_index_from_text,
        trace::{CapabilityUnavailable, TraceUnavailableReason},
    };

    fn input(profile: MacroProfileId) -> MacroDbInput {
        MacroDbInput {
            profile,
            roots: Vec::new(),
            files: Vec::new(),
            predefines: Vec::new(),
            literal_includes: Vec::new(),
        }
    }

    fn file_input(file_id: FileId, text: &str) -> FileMacroInput {
        FileMacroInput {
            file_id,
            index: preproc_file_index_from_text(
                text,
                &SyntaxTreeOptions::without_include_expansion(),
            ),
        }
    }

    fn db_from_text(text: &str) -> MacroDb {
        db_from_text_with_predefines(text, Vec::new())
    }

    fn db_from_text_with_predefines(text: &str, predefines: Vec<MacroPredefine>) -> MacroDb {
        MacroDb::new(MacroDbInput {
            profile: MacroProfileId(1),
            roots: vec![FileId(0)],
            files: vec![file_input(FileId(0), text)],
            predefines,
            literal_includes: Vec::new(),
        })
    }

    fn usage_resolution(db: &MacroDb, id: u32) -> &MacroUseResolution {
        &db.macro_use(MacroUseId(id)).unwrap().resolution
    }

    fn offset(text: &str, needle: &str) -> TextSize {
        TextSize::from(text.find(needle).unwrap() as u32)
    }

    fn last_offset(text: &str, needle: &str) -> TextSize {
        TextSize::from(text.rfind(needle).unwrap() as u32)
    }

    #[test]
    fn macro_db_key_is_profile_aware() {
        let profile_a = MacroDb::new(input(MacroProfileId(1)));
        let profile_b = MacroDb::new(input(MacroProfileId(2)));

        assert_eq!(profile_a.profile(), MacroProfileId(1));
        assert_eq!(profile_b.profile(), MacroProfileId(2));
        assert_ne!(profile_a.profile(), profile_b.profile());
    }

    #[test]
    fn replays_same_file_define_before_use() {
        let db = db_from_text("`define WIDTH 8\n`WIDTH\n");

        assert_eq!(usage_resolution(&db, 0), &MacroUseResolution::Resolved(MacroDefId(0)));
    }

    #[test]
    fn redefinition_replaces_visible_definition() {
        let db = db_from_text("`define WIDTH 8\n`define WIDTH 16\n`WIDTH\n");

        assert_eq!(db.definitions().len(), 2);
        assert_eq!(usage_resolution(&db, 0), &MacroUseResolution::Resolved(MacroDefId(1)));
    }

    #[test]
    fn undef_removes_visible_definition() {
        let db = db_from_text("`define WIDTH 8\n`undef WIDTH\n`WIDTH\n");

        assert_eq!(usage_resolution(&db, 0), &MacroUseResolution::Unresolved);
    }

    #[test]
    fn profile_predefine_is_visible_as_virtual_source() {
        let db = db_from_text_with_predefines(
            "`WIDTH\n",
            vec![MacroPredefine {
                name: MacroName::new("WIDTH"),
                value: Some(SmolStr::new("8")),
                source: PredefineSource::CommandLine,
            }],
        );

        assert_eq!(usage_resolution(&db, 0), &MacroUseResolution::Resolved(MacroDefId(0)));
        assert_eq!(
            db.definition(MacroDefId(0)).unwrap().origin,
            SourceOrigin::VirtualPredefine {
                profile: MacroProfileId(1),
                name: MacroName::new("WIDTH"),
                source: PredefineSource::CommandLine,
            }
        );
    }

    #[test]
    fn macro_env_at_replays_to_requested_offset() {
        let text = "`define A 1\n`define B 1\n`undef A\n";
        let db = db_from_text(text);
        let after_b = TextSize::from(text.find("`undef").unwrap() as u32);

        let visible = db.macro_env_at(FileId(0), after_b).unwrap();

        assert_eq!(
            visible.iter().map(|source| source.name.as_str()).collect::<Vec<_>>(),
            vec!["A", "B"]
        );
    }

    #[test]
    fn macro_definition_at_returns_visible_definition() {
        let text = "`define WIDTH 8\nlogic [`WIDTH-1:0] data;\n";
        let db = db_from_text(text);

        let result = db.macro_definition_at(FileId(0), last_offset(text, "WIDTH"));

        assert_eq!(
            result,
            MacroDefinitionAtResult::Definition(MacroSource {
                id: MacroDefId(0),
                name: MacroName::new("WIDTH"),
                origin: SourceOrigin::File {
                    file_id: FileId(0),
                    range: TextRange::new(TextSize::from(8), TextSize::from(13)),
                },
            })
        );
    }

    #[test]
    fn macro_definition_at_does_not_match_use_end_offset() {
        let text = "`define WIDTH 8\nlogic [`WIDTH-1:0] data;\n";
        let db = db_from_text(text);
        let use_range = db.macro_use(MacroUseId(0)).unwrap().range.unwrap();

        assert_eq!(
            db.macro_definition_at(FileId(0), use_range.end()),
            MacroDefinitionAtResult::NoMacroUse
        );
    }

    #[test]
    fn macro_definition_at_returns_unresolved_reason() {
        let text = "logic [`WIDTH-1:0] data;\n";
        let db = db_from_text(text);

        let result = db.macro_definition_at(FileId(0), offset(text, "WIDTH"));

        assert_eq!(
            result,
            MacroDefinitionAtResult::Failed(MacroQueryFailure::Unresolved {
                name: MacroName::new("WIDTH"),
            })
        );
    }

    #[test]
    fn macro_definition_at_fails_closed_in_inactive_branch() {
        let text = "`ifdef USE_A\n`WIDTH\n`endif\n";
        let db = db_from_text(text);

        let result = db.macro_definition_at(FileId(0), offset(text, "WIDTH"));

        assert!(matches!(
            result,
            MacroDefinitionAtResult::Failed(MacroQueryFailure::InactiveBranch {
                file_id: FileId(0),
                ..
            })
        ));
    }

    #[test]
    fn macro_references_return_resolved_uses_only() {
        let text = "`define WIDTH 8\n`WIDTH\n`undef WIDTH\n`define WIDTH 16\n`WIDTH\n";
        let db = db_from_text(text);

        let first = db.macro_references(MacroDefId(0));
        let second = db.macro_references(MacroDefId(1));

        assert_eq!(
            first,
            MacroReferencesResult::References(vec![MacroReference {
                use_id: MacroUseId(0),
                file_id: FileId(0),
                range: db.macro_use(MacroUseId(0)).unwrap().range,
            }])
        );
        assert_eq!(
            second,
            MacroReferencesResult::References(vec![MacroReference {
                use_id: MacroUseId(1),
                file_id: FileId(0),
                range: db.macro_use(MacroUseId(1)).unwrap().range,
            }])
        );
    }

    #[test]
    fn macro_references_at_definition_uses_definition_name_source() {
        let text = "`define WIDTH 8\n`WIDTH\n";
        let db = db_from_text(text);

        let references = db.macro_references_at_definition(FileId(0), offset(text, "WIDTH"));

        assert_eq!(
            references,
            MacroReferencesResult::References(vec![MacroReference {
                use_id: MacroUseId(0),
                file_id: FileId(0),
                range: db.macro_use(MacroUseId(0)).unwrap().range,
            }])
        );
    }

    #[test]
    fn macro_references_at_definition_does_not_match_definition_end_offset() {
        let text = "`define WIDTH 8\n`WIDTH\n";
        let db = db_from_text(text);
        let SourceOrigin::File { range, .. } = db.definition(MacroDefId(0)).unwrap().origin else {
            panic!("test definition should come from source file");
        };

        assert!(matches!(
            db.macro_references_at_definition(FileId(0), range.end()),
            MacroReferencesResult::Failed(MacroQueryFailure::CapabilityUnavailable {
                capability,
                ..
            }) if capability == "macro_references_at_definition"
        ));
    }

    #[test]
    fn literal_include_replay_makes_definition_visible() {
        let root = "`include \"defs.svh\"\nlogic [`WIDTH-1:0] data;\n";
        let header = "`define WIDTH 8\n";
        let db = MacroDb::new(MacroDbInput {
            profile: MacroProfileId(1),
            roots: vec![FileId(0)],
            files: vec![file_input(FileId(0), root), file_input(FileId(1), header)],
            predefines: Vec::new(),
            literal_includes: vec![LiteralIncludeInput {
                from_file: FileId(0),
                include_index: 0,
                to_file: FileId(1),
            }],
        });

        let result = db.macro_definition_at(FileId(0), offset(root, "WIDTH"));

        assert_eq!(
            result,
            MacroDefinitionAtResult::Definition(MacroSource {
                id: MacroDefId(0),
                name: MacroName::new("WIDTH"),
                origin: SourceOrigin::File {
                    file_id: FileId(1),
                    range: TextRange::new(TextSize::from(8), TextSize::from(13)),
                },
            })
        );
    }

    #[test]
    fn literal_include_without_edge_fails_closed() {
        let root = "`include \"defs.svh\"\nlogic [`WIDTH-1:0] data;\n";
        let db = MacroDb::new(MacroDbInput {
            profile: MacroProfileId(1),
            roots: vec![FileId(0)],
            files: vec![file_input(FileId(0), root)],
            predefines: Vec::new(),
            literal_includes: Vec::new(),
        });

        let result = db.macro_definition_at(FileId(0), offset(root, "WIDTH"));

        assert!(matches!(
            result,
            MacroDefinitionAtResult::Failed(MacroQueryFailure::CapabilityUnavailable {
                capability,
                ..
            }) if capability == "literal_include_replay"
        ));
    }

    #[test]
    fn nested_literal_include_failure_propagates_to_includer() {
        let root = "`include \"a.svh\"\nlogic [`WIDTH-1:0] data;\n";
        let header = "`include \"missing.svh\"\n";
        let db = MacroDb::new(MacroDbInput {
            profile: MacroProfileId(1),
            roots: vec![FileId(0)],
            files: vec![file_input(FileId(0), root), file_input(FileId(1), header)],
            predefines: Vec::new(),
            literal_includes: vec![LiteralIncludeInput {
                from_file: FileId(0),
                include_index: 0,
                to_file: FileId(1),
            }],
        });

        let result = db.macro_definition_at(FileId(0), offset(root, "WIDTH"));

        assert!(matches!(
            result,
            MacroDefinitionAtResult::Failed(MacroQueryFailure::CapabilityUnavailable {
                capability,
                ..
            }) if capability == "literal_include_replay"
        ));
    }

    #[test]
    fn include_target_at_uses_supplied_literal_edge() {
        let root = "`include \"defs.svh\"\n";
        let db = MacroDb::new(MacroDbInput {
            profile: MacroProfileId(1),
            roots: vec![FileId(0)],
            files: vec![file_input(FileId(0), root), file_input(FileId(1), "`define WIDTH 8\n")],
            predefines: Vec::new(),
            literal_includes: vec![LiteralIncludeInput {
                from_file: FileId(0),
                include_index: 0,
                to_file: FileId(1),
            }],
        });

        assert_eq!(
            db.include_target_at(FileId(0), offset(root, "defs")),
            IncludeTargetAtResult::Target(FileId(1))
        );
    }

    #[test]
    fn include_target_at_does_not_match_directive_end_offset() {
        let root = "`include \"defs.svh\"\n";
        let db = MacroDb::new(MacroDbInput {
            profile: MacroProfileId(1),
            roots: vec![FileId(0)],
            files: vec![file_input(FileId(0), root), file_input(FileId(1), "`define WIDTH 8\n")],
            predefines: Vec::new(),
            literal_includes: vec![LiteralIncludeInput {
                from_file: FileId(0),
                include_index: 0,
                to_file: FileId(1),
            }],
        });
        let include_end = db.files()[0].index.directives[0].range.unwrap().end();

        assert_eq!(db.include_target_at(FileId(0), include_end), IncludeTargetAtResult::NoInclude);
    }

    #[test]
    fn include_target_at_fails_closed_without_literal_edge() {
        let root = "`include \"defs.svh\"\n";
        let db = MacroDb::new(MacroDbInput {
            profile: MacroProfileId(1),
            roots: vec![FileId(0)],
            files: vec![file_input(FileId(0), root)],
            predefines: Vec::new(),
            literal_includes: Vec::new(),
        });

        assert!(matches!(
            db.include_target_at(FileId(0), offset(root, "defs")),
            IncludeTargetAtResult::Failed(MacroQueryFailure::CapabilityUnavailable {
                capability,
                ..
            }) if capability == "literal_include_replay"
        ));
    }

    #[test]
    fn trace_capability_reports_missing_preproc_trace() {
        let db = db_from_text("`define WIDTH 8\n`WIDTH\n");

        assert_eq!(
            db.trace_capability(),
            TraceCapability::CapabilityUnavailable(CapabilityUnavailable {
                capability: SmolStr::new(PREPROC_TRACE_CAPABILITY),
                reason: TraceUnavailableReason::MissingPreprocTrace,
            })
        );
    }

    #[test]
    fn expansion_for_use_requires_preproc_trace() {
        let db = db_from_text("`define WIDTH 8\n`WIDTH\n");

        assert_eq!(
            db.expansion_for_use(MacroUseId(0)),
            PreprocTraceResult::CapabilityUnavailable(CapabilityUnavailable {
                capability: SmolStr::new(PREPROC_TRACE_CAPABILITY),
                reason: TraceUnavailableReason::MissingPreprocTrace,
            })
        );
    }

    #[test]
    fn origin_for_expanded_token_requires_preproc_trace() {
        let db = db_from_text("`define WIDTH 8\n`WIDTH\n");

        assert_eq!(
            db.origin_for_expanded_token(ExpandedTokenId(99)),
            PreprocTraceResult::CapabilityUnavailable(CapabilityUnavailable {
                capability: SmolStr::new(PREPROC_TRACE_CAPABILITY),
                reason: TraceUnavailableReason::MissingPreprocTrace,
            })
        );
    }

    #[test]
    fn token_include_target_requires_preproc_trace() {
        let root = "`include `HEADER\n";
        let db = MacroDb::new(MacroDbInput {
            profile: MacroProfileId(1),
            roots: vec![FileId(0)],
            files: vec![file_input(FileId(0), root)],
            predefines: Vec::new(),
            literal_includes: Vec::new(),
        });

        assert!(matches!(
            db.include_target_at(FileId(0), offset(root, "HEADER")),
            IncludeTargetAtResult::Failed(MacroQueryFailure::CapabilityUnavailable {
                capability,
                reason,
            }) if capability == PREPROC_TRACE_CAPABILITY && reason == "MissingPreprocTrace"
        ));
    }

    #[test]
    fn macro_expanded_include_replay_requires_preproc_trace() {
        let root = "`define HEADER \"defs.svh\"\n`include `HEADER\n`WIDTH\n";
        let db = MacroDb::new(MacroDbInput {
            profile: MacroProfileId(1),
            roots: vec![FileId(0)],
            files: vec![file_input(FileId(0), root)],
            predefines: Vec::new(),
            literal_includes: Vec::new(),
        });

        assert!(matches!(
            db.macro_definition_at(FileId(0), offset(root, "WIDTH")),
            MacroDefinitionAtResult::Failed(MacroQueryFailure::CapabilityUnavailable {
                capability,
                reason,
            }) if capability == PREPROC_TRACE_CAPABILITY && reason == "MissingPreprocTrace"
        ));
    }
}
