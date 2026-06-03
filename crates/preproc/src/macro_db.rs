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
pub enum MacroDefinitionAtResult {
    Definition(MacroSource),
    NoMacroUse,
    Failed(MacroQueryFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroReferencesResult {
    References(Vec<MacroUseId>),
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
    let origin =
        define.range.map(|range| SourceOrigin::File { file_id, range }).unwrap_or_else(|| {
            SourceOrigin::Unsupported { reason: SmolStr::new("macro define has no source range") }
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
}
