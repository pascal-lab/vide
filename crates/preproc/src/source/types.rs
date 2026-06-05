use std::collections::BTreeMap;

use smol_str::SmolStr;
use utils::line_index::{TextRange, TextSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PreprocSourceId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroEventKind {
    Define,
    Undef,
    Include,
    Conditional,
    Branch,
    Usage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroIncludeTarget {
    Literal { path: SmolStr, raw: SmolStr },
    Token { raw: SmolStr },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroConditionalKind {
    IfDef,
    IfNDef,
    ElsIf,
    Else,
    EndIf,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourcePreprocEventId(pub(super) u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocSource {
    pub id: PreprocSourceId,
    pub path: SmolStr,
    pub origin: PreprocSourceOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreprocSourceOrigin {
    Root,
    Included { include_event_id: SourcePreprocEventId },
    Predefine,
    Detached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceIncludeEdge {
    pub include_event_id: SourcePreprocEventId,
    pub included_source: PreprocSourceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceIncludeChainEntry {
    pub include_event_id: SourcePreprocEventId,
    pub include_range: SourceRange,
    pub included_source: PreprocSourceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourcePreprocIndex {
    pub root_source: Option<PreprocSourceId>,
    pub sources: Vec<PreprocSource>,
    pub include_edges: Vec<SourceIncludeEdge>,
    pub event_records: Vec<SourcePreprocEventRecord>,
    pub defines: Vec<SourceMacroDefine>,
    pub undefs: Vec<SourceMacroUndef>,
    pub includes: Vec<SourceMacroInclude>,
    pub conditionals: Vec<SourceMacroConditional>,
    pub usages: Vec<SourceMacroUsage>,
    pub inactive_ranges: Vec<SourceRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePreprocEventRecord {
    pub event_id: SourcePreprocEventId,
    pub kind: MacroEventKind,
    pub range: SourceRange,
    pub index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroDefine {
    pub event_id: SourcePreprocEventId,
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
    pub event_id: SourcePreprocEventId,
    pub name: Option<SmolStr>,
    pub name_range: Option<SourceRange>,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroInclude {
    pub event_id: SourcePreprocEventId,
    pub target: MacroIncludeTarget,
    pub target_range: Option<SourceRange>,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroConditional {
    pub event_id: SourcePreprocEventId,
    pub kind: MacroConditionalKind,
    pub expr: Vec<SourceMacroToken>,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroUsage {
    pub event_id: SourcePreprocEventId,
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
    pub(super) index: SourcePreprocIndex,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceMacroEnvironment {
    pub(super) definitions: BTreeMap<SmolStr, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroBinding<'a> {
    pub name: SmolStr,
    pub event_id: SourcePreprocEventId,
    pub define_index: usize,
    pub define: &'a SourceMacroDefine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroResolution<'a> {
    pub usage_index: usize,
    pub usage: &'a SourceMacroUsage,
    pub definition: SourceMacroBinding<'a>,
    pub definition_provenance: SourcePreprocProvenance,
    pub definition_include_chain: Vec<SourceIncludeChainEntry>,
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
    pub event_id: SourcePreprocEventId,
    pub entity: SourcePreprocEntity,
    pub name: Option<SmolStr>,
    pub range: SourceRange,
    pub name_range: Option<SourceRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourcePreprocEvent<'a> {
    Define {
        source_order: usize,
        event_id: SourcePreprocEventId,
        index: usize,
        define: &'a SourceMacroDefine,
    },
    Undef {
        source_order: usize,
        event_id: SourcePreprocEventId,
        index: usize,
        undef: &'a SourceMacroUndef,
    },
    Include {
        source_order: usize,
        event_id: SourcePreprocEventId,
        index: usize,
        include: &'a SourceMacroInclude,
    },
    Conditional {
        source_order: usize,
        event_id: SourcePreprocEventId,
        index: usize,
        conditional: &'a SourceMacroConditional,
    },
    Branch {
        source_order: usize,
        event_id: SourcePreprocEventId,
        index: usize,
        conditional: &'a SourceMacroConditional,
    },
    Usage {
        source_order: usize,
        event_id: SourcePreprocEventId,
        index: usize,
        usage: &'a SourceMacroUsage,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcePreprocError {
    MissingRootSource,
    MissingEventRange { source_order: usize, kind: MacroEventKind },
    MissingEvent { event_id: u32 },
    MissingIncludedSource { include_event_id: u32, source: u32 },
    MissingIncludeEvent { include_event_id: u32 },
    IncludeEdgeNotInclude { include_event_id: u32 },
    MissingIncludeEdge { source: u32 },
    IncludeCycle { source: u32 },
}

impl PreprocSourceId {
    pub fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

impl SourcePreprocEventId {
    pub fn raw(self) -> u32 {
        self.0
    }
}

impl From<u32> for PreprocSourceId {
    fn from(value: u32) -> Self {
        Self::new(value)
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

impl SourcePreprocEvent<'_> {
    pub fn event_id(&self) -> SourcePreprocEventId {
        match self {
            SourcePreprocEvent::Define { event_id, .. }
            | SourcePreprocEvent::Undef { event_id, .. }
            | SourcePreprocEvent::Include { event_id, .. }
            | SourcePreprocEvent::Conditional { event_id, .. }
            | SourcePreprocEvent::Branch { event_id, .. }
            | SourcePreprocEvent::Usage { event_id, .. } => *event_id,
        }
    }

    pub fn source_order(&self) -> usize {
        match self {
            SourcePreprocEvent::Define { source_order, .. }
            | SourcePreprocEvent::Undef { source_order, .. }
            | SourcePreprocEvent::Include { source_order, .. }
            | SourcePreprocEvent::Conditional { source_order, .. }
            | SourcePreprocEvent::Branch { source_order, .. }
            | SourcePreprocEvent::Usage { source_order, .. } => *source_order,
        }
    }

    pub fn kind(&self) -> MacroEventKind {
        match self {
            SourcePreprocEvent::Define { .. } => MacroEventKind::Define,
            SourcePreprocEvent::Undef { .. } => MacroEventKind::Undef,
            SourcePreprocEvent::Include { .. } => MacroEventKind::Include,
            SourcePreprocEvent::Conditional { .. } => MacroEventKind::Conditional,
            SourcePreprocEvent::Branch { .. } => MacroEventKind::Branch,
            SourcePreprocEvent::Usage { .. } => MacroEventKind::Usage,
        }
    }

    pub fn entity(&self) -> SourcePreprocEntity {
        match self {
            SourcePreprocEvent::Define { index, .. } => SourcePreprocEntity::Define(*index),
            SourcePreprocEvent::Undef { index, .. } => SourcePreprocEntity::Undef(*index),
            SourcePreprocEvent::Include { index, .. } => SourcePreprocEntity::Include(*index),
            SourcePreprocEvent::Conditional { index, .. }
            | SourcePreprocEvent::Branch { index, .. } => SourcePreprocEntity::Conditional(*index),
            SourcePreprocEvent::Usage { index, .. } => SourcePreprocEntity::Usage(*index),
        }
    }

    pub fn range(&self) -> SourceRange {
        match self {
            SourcePreprocEvent::Define { define, .. } => define.range,
            SourcePreprocEvent::Undef { undef, .. } => undef.range,
            SourcePreprocEvent::Include { include, .. } => include.range,
            SourcePreprocEvent::Conditional { conditional, .. }
            | SourcePreprocEvent::Branch { conditional, .. } => conditional.range,
            SourcePreprocEvent::Usage { usage, .. } => usage.range,
        }
    }
}
