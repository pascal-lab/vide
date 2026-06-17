use smol_str::SmolStr;
use syntax::TokenKind;
pub use syntax::preproc::TokenOrigin;

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
    pub provenance: TokenOrigin,
}
