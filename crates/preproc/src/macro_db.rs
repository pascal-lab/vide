use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use crate::directive_index::{
    MacroDefine, MacroDirective, MacroDirectiveKind, MacroUsage, PreprocFileIndex,
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
pub struct MacroDbInput {
    pub profile: MacroProfileId,
    pub files: Vec<FileMacroInput>,
    pub predefines: Vec<MacroPredefine>,
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
pub struct MacroDb {
    profile: MacroProfileId,
    files: Vec<FileMacroInput>,
    predefines: Vec<MacroPredefine>,
    definitions: Vec<MacroSource>,
    uses: Vec<MacroUse>,
    env_snapshots: Vec<EnvSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EnvSnapshot {
    file_id: FileId,
    offset: TextSize,
    visible: Vec<MacroDefId>,
}

impl MacroDb {
    pub fn new(input: MacroDbInput) -> Self {
        let MacroDbInput { profile, files, predefines } = input;
        let mut definitions = Vec::new();
        let mut uses = Vec::new();
        let mut env_snapshots = Vec::new();
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

        for file in &files {
            replay_file(file, &predefine_env, &mut definitions, &mut uses, &mut env_snapshots);
        }

        Self { profile, files, predefines, definitions, uses, env_snapshots }
    }

    pub fn profile(&self) -> MacroProfileId {
        self.profile
    }

    pub fn files(&self) -> &[FileMacroInput] {
        &self.files
    }

    pub fn predefines(&self) -> &[MacroPredefine] {
        &self.predefines
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

        let Some(definition) = self.definition_at_source(file_id, offset) else {
            return MacroReferencesResult::Failed(MacroQueryFailure::CapabilityUnavailable {
                capability: SmolStr::new("macro_references_at_definition"),
                reason: SmolStr::new("offset does not point at a MacroDb definition source"),
            });
        };

        self.macro_references(definition)
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
}

fn replay_file(
    file: &FileMacroInput,
    predefine_env: &FxHashMap<MacroName, MacroDefId>,
    definitions: &mut Vec<MacroSource>,
    uses: &mut Vec<MacroUse>,
    env_snapshots: &mut Vec<EnvSnapshot>,
) {
    let mut env = predefine_env.clone();
    push_env_snapshot(file.file_id, TextSize::from(0), &env, env_snapshots);

    for directive in &file.index.directives {
        match directive.kind {
            MacroDirectiveKind::Define => {
                if let Some(source) = file.index.defines.get(directive.index).and_then(|define| {
                    macro_source_from_define(file.file_id, define, definitions.len())
                }) {
                    env.insert(source.name.clone(), source.id);
                    definitions.push(source);
                }
            }
            MacroDirectiveKind::Undef => {
                if let Some(undef) = file.index.undefs.get(directive.index)
                    && let Some(name) = &undef.name
                {
                    env.remove(&MacroName::new(name.clone()));
                }
            }
            MacroDirectiveKind::Usage => {
                if let Some(macro_use) =
                    file.index.usages.get(directive.index).and_then(|usage| {
                        macro_use_from_usage(file.file_id, usage, &env, uses.len())
                    })
                {
                    uses.push(macro_use);
                }
            }
            MacroDirectiveKind::Include
            | MacroDirectiveKind::Conditional
            | MacroDirectiveKind::Branch => {}
        }
        push_env_snapshot(file.file_id, directive_offset(directive), &env, env_snapshots);
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

fn range_contains(range: Option<TextRange>, offset: TextSize) -> bool {
    range.is_some_and(|range| range.start() <= offset && offset <= range.end())
}

#[cfg(test)]
mod tests {
    use syntax::SyntaxTreeOptions;

    use super::*;
    use crate::directive_index::preproc_file_index_from_text;

    fn input(profile: MacroProfileId) -> MacroDbInput {
        MacroDbInput { profile, files: Vec::new(), predefines: Vec::new() }
    }

    fn db_from_text(text: &str) -> MacroDb {
        db_from_text_with_predefines(text, Vec::new())
    }

    fn db_from_text_with_predefines(text: &str, predefines: Vec<MacroPredefine>) -> MacroDb {
        let index =
            preproc_file_index_from_text(text, &SyntaxTreeOptions::without_include_expansion());
        MacroDb::new(MacroDbInput {
            profile: MacroProfileId(1),
            files: vec![FileMacroInput { file_id: FileId(0), index }],
            predefines,
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
}
