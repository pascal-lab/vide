use smol_str::SmolStr;
use utils::line_index::TextRange;
use vfs::FileId;

use crate::directive_index::PreprocFileIndex;

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
}

impl MacroDb {
    pub fn new(input: MacroDbInput) -> Self {
        Self { profile: input.profile, files: input.files, predefines: input.predefines }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(profile: MacroProfileId) -> MacroDbInput {
        MacroDbInput { profile, files: Vec::new(), predefines: Vec::new() }
    }

    #[test]
    fn macro_db_key_is_profile_aware() {
        let profile_a = MacroDb::new(input(MacroProfileId(1)));
        let profile_b = MacroDb::new(input(MacroProfileId(2)));

        assert_eq!(profile_a.profile(), MacroProfileId(1));
        assert_eq!(profile_b.profile(), MacroProfileId(2));
        assert_ne!(profile_a.profile(), profile_b.profile());
    }
}
