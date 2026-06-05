use std::collections::BTreeMap;

use smol_str::SmolStr;
use utils::line_index::{TextRange, TextSize};

use crate::index::{MacroConditionalKind, MacroDirectiveKind, MacroIncludeTarget};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PreprocSourceId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    pub source: PreprocSourceId,
    pub offset: TextSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceRange {
    pub source: PreprocSourceId,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocSource {
    pub id: PreprocSourceId,
    pub path: SmolStr,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourcePreprocIndex {
    pub root_source: Option<PreprocSourceId>,
    pub sources: Vec<PreprocSource>,
    pub directives: Vec<SourceMacroDirective>,
    pub defines: Vec<SourceMacroDefine>,
    pub undefs: Vec<SourceMacroUndef>,
    pub includes: Vec<SourceMacroInclude>,
    pub conditionals: Vec<SourceMacroConditional>,
    pub usages: Vec<SourceMacroUsage>,
    pub inactive_ranges: Vec<SourceRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroDirective {
    pub kind: MacroDirectiveKind,
    pub range: SourceRange,
    pub index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroDefine {
    pub name: Option<SmolStr>,
    pub name_range: Option<SourceRange>,
    pub params: Option<Vec<SourceMacroParam>>,
    pub body: Vec<SourceMacroToken>,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroParam {
    pub name: Option<SmolStr>,
    pub name_range: Option<SourceRange>,
    pub default: Option<Vec<SourceMacroToken>>,
    pub range: Option<SourceRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroUndef {
    pub name: Option<SmolStr>,
    pub name_range: Option<SourceRange>,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroInclude {
    pub target: MacroIncludeTarget,
    pub target_range: Option<SourceRange>,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroConditional {
    pub kind: MacroConditionalKind,
    pub expr: Vec<SourceMacroToken>,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroUsage {
    pub name: Option<SmolStr>,
    pub name_range: Option<SourceRange>,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroToken {
    pub raw: SmolStr,
    pub value: SmolStr,
    pub range: Option<SourceRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePreprocModel {
    index: SourcePreprocIndex,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceMacroEnvironment {
    definitions: BTreeMap<SmolStr, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroBinding<'a> {
    pub name: SmolStr,
    pub define_index: usize,
    pub define: &'a SourceMacroDefine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourcePreprocEntity {
    Define(usize),
    Undef(usize),
    Usage(usize),
    Include(usize),
    Conditional(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePreprocProvenance {
    pub entity: SourcePreprocEntity,
    pub name: Option<SmolStr>,
    pub range: SourceRange,
    pub name_range: Option<SourceRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourcePreprocEvent<'a> {
    Define { source_order: usize, index: usize, define: &'a SourceMacroDefine },
    Undef { source_order: usize, index: usize, undef: &'a SourceMacroUndef },
    Include { source_order: usize, index: usize, include: &'a SourceMacroInclude },
    Conditional { source_order: usize, index: usize, conditional: &'a SourceMacroConditional },
    Branch { source_order: usize, index: usize, conditional: &'a SourceMacroConditional },
    Usage { source_order: usize, index: usize, usage: &'a SourceMacroUsage },
}

impl PreprocSourceId {
    pub fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

impl From<u32> for PreprocSourceId {
    fn from(value: u32) -> Self {
        Self::new(value)
    }
}

impl SourcePreprocModel {
    pub fn new(index: SourcePreprocIndex) -> Self {
        Self { index }
    }

    pub fn index(&self) -> &SourcePreprocIndex {
        &self.index
    }

    pub fn into_index(self) -> SourcePreprocIndex {
        self.index
    }
}

impl SourceMacroEnvironment {
    pub fn define_index(&self, name: &str) -> Option<usize> {
        self.definitions.get(name).copied()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.definitions.contains_key(name)
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    pub fn names(&self) -> impl Iterator<Item = &SmolStr> {
        self.definitions.keys()
    }

    pub fn definitions(&self) -> &BTreeMap<SmolStr, usize> {
        &self.definitions
    }
}
