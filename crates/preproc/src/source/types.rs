use smol_str::SmolStr;
use syntax::TokenKind;
use utils::line_index::{TextRange, TextSize};

use super::provenance::SourcePreprocTables;

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

macro_rules! source_identity_key {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u32);

        impl $name {
            pub fn new(raw: u32) -> Self {
                Self(raw)
            }

            pub fn raw(self) -> u32 {
                self.0
            }
        }
    };
}

source_identity_key!(SourceMacroDefinitionKey);
source_identity_key!(SourceMacroCallKey);
source_identity_key!(SourceMacroExpansionKey);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceMacroBodyIdentity {
    pub call: SourceMacroCallKey,
    pub definition: SourceMacroDefinitionKey,
    pub expansion: SourceMacroExpansionKey,
    pub parent_expansion: Option<SourceMacroExpansionKey>,
    pub body_token_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceMacroArgumentIdentity {
    pub call: SourceMacroCallKey,
    pub definition: SourceMacroDefinitionKey,
    pub expansion: SourceMacroExpansionKey,
    pub parent_expansion: Option<SourceMacroExpansionKey>,
    pub body_token_index: usize,
    pub argument_index: usize,
    pub argument_token_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceMacroBuiltinIdentity {
    pub call: SourceMacroCallKey,
    pub expansion: SourceMacroExpansionKey,
    pub parent_expansion: Option<SourceMacroExpansionKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceMacroOperationIdentity {
    pub call: SourceMacroCallKey,
    pub definition: SourceMacroDefinitionKey,
    pub expansion: SourceMacroExpansionKey,
    pub parent_expansion: Option<SourceMacroExpansionKey>,
}

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
    pub emitted_tokens: Vec<SourceEmittedTokenFact>,
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
    pub identity: Option<SourceMacroDefinitionKey>,
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
    pub identity: Option<SourceMacroCallKey>,
    pub definition_identity: Option<SourceMacroDefinitionKey>,
    pub expansion_identity: Option<SourceMacroExpansionKey>,
    pub parent_expansion_identity: Option<SourceMacroExpansionKey>,
    pub name: Option<SmolStr>,
    pub name_range: Option<SourceRange>,
    pub arguments: Vec<SourceMacroActualArgument>,
    pub range: SourceRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroActualArgument {
    pub argument_index: usize,
    pub argument_range: Option<SourceRange>,
    pub tokens: Vec<SourceMacroToken>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroToken {
    pub raw: SmolStr,
    pub value: SmolStr,
    pub range: Option<SourceRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceTokenKind {
    Unknown,
    Syntax(TokenKind),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEmittedTokenFact {
    pub raw: SmolStr,
    pub value: SmolStr,
    pub display: SmolStr,
    pub kind: SourceTokenKind,
    pub provenance: SourceTokenProvenanceFact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceTokenProvenanceFact {
    Source {
        token_range: SourceRange,
    },
    MacroBody {
        macro_name: SmolStr,
        identity: Option<SourceMacroBodyIdentity>,
        call_range: SourceRange,
        body_token_range: SourceRange,
    },
    MacroArgument {
        macro_name: SmolStr,
        identity: Option<SourceMacroArgumentIdentity>,
        call_range: SourceRange,
        body_token_range: SourceRange,
        argument_token_range: SourceRange,
    },
    Builtin {
        name: SmolStr,
        identity: Option<SourceMacroBuiltinIdentity>,
    },
    TokenPaste {
        identity: Option<SourceMacroOperationIdentity>,
    },
    Stringification {
        identity: Option<SourceMacroOperationIdentity>,
    },
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePreprocModel {
    pub(super) index: SourcePreprocIndex,
    pub(super) tables: SourcePreprocTables,
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
