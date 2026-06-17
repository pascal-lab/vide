use smol_str::SmolStr;
use syntax::TokenKind;

use super::*;

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
