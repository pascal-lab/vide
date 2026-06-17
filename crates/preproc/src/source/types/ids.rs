use smol_str::SmolStr;
pub use syntax::preproc::{MacroCallId, MacroDefinitionId, MacroExpansionId};
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
pub struct SourcePreprocEventId(pub(in crate::source) u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceMacroBodyIdentity {
    pub call: MacroCallId,
    pub definition: MacroDefinitionId,
    pub expansion: MacroExpansionId,
    pub parent_expansion: Option<MacroExpansionId>,
    pub body_token_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceMacroArgumentIdentity {
    pub call: MacroCallId,
    pub definition: MacroDefinitionId,
    pub expansion: MacroExpansionId,
    pub parent_expansion: Option<MacroExpansionId>,
    pub body_token_index: usize,
    pub argument_index: usize,
    pub argument_token_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceMacroBuiltinIdentity {
    pub call: MacroCallId,
    pub expansion: MacroExpansionId,
    pub parent_expansion: Option<MacroExpansionId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceMacroOperationIdentity {
    pub call: MacroCallId,
    pub definition: MacroDefinitionId,
    pub expansion: MacroExpansionId,
    pub parent_expansion: Option<MacroExpansionId>,
    pub body_token_index: usize,
    pub argument_index: Option<usize>,
    pub argument_token_index: Option<usize>,
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
