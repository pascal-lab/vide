use smol_str::SmolStr;

use crate::ids::{
    MacroCallIdentity, MacroDefinitionIdentity, MacroExpansionIdentity, OriginId, SpanId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacroBodyTokenIdentity {
    pub call: MacroCallIdentity,
    pub definition: MacroDefinitionIdentity,
    pub expansion: MacroExpansionIdentity,
    pub parent_expansion: Option<MacroExpansionIdentity>,
    pub body_token_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacroArgumentTokenIdentity {
    pub call: MacroCallIdentity,
    pub definition: MacroDefinitionIdentity,
    pub expansion: MacroExpansionIdentity,
    pub parent_expansion: Option<MacroExpansionIdentity>,
    pub body_token_index: usize,
    pub argument_index: usize,
    pub argument_token_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacroOperationTokenIdentity {
    pub call: MacroCallIdentity,
    pub definition: MacroDefinitionIdentity,
    pub expansion: MacroExpansionIdentity,
    pub parent_expansion: Option<MacroExpansionIdentity>,
    pub body_token_index: usize,
    pub argument_index: Option<usize>,
    pub argument_token_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SourceOrigin {
    Written {
        span: SpanId,
    },
    MacroBody {
        identity: MacroBodyTokenIdentity,
        body_span: SpanId,
        call_span: SpanId,
        emitted_span: SpanId,
    },
    MacroArgument {
        identity: MacroArgumentTokenIdentity,
        argument_span: SpanId,
        body_param_span: SpanId,
        call_span: SpanId,
        emitted_span: SpanId,
    },
    TokenPaste {
        identity: MacroOperationTokenIdentity,
        inputs: Vec<SpanId>,
        call_span: SpanId,
        emitted_span: SpanId,
    },
    Stringification {
        identity: MacroOperationTokenIdentity,
        inputs: Vec<SpanId>,
        call_span: SpanId,
        emitted_span: SpanId,
    },
    Builtin {
        name: SmolStr,
        call_span: SpanId,
        emitted_span: SpanId,
    },
    Synthetic {
        reason: SyntheticReason,
        preferred_span: Option<SpanId>,
    },
    Composite {
        origins: Vec<OriginId>,
        preferred_span: Option<SpanId>,
    },
    Unavailable {
        reason: crate::SourceUnavailable,
    },
    Alias {
        origin: OriginId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SyntheticReason {
    MissingSyntax,
    LoweredImplicitConstruct,
    GeneratedCompletionContext,
    Recovery,
    Other(SmolStr),
}
