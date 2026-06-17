use smol_str::SmolStr;

use super::*;

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
