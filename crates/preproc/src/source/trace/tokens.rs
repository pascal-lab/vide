use super::*;

pub(super) fn emitted_token_from_trace(token: EmittedToken) -> SourceEmittedTokenRecord {
    SourceEmittedTokenRecord {
        raw: token.raw_text.to_smolstr(),
        value: token.value_text.to_smolstr(),
        display: token.display_text.to_smolstr(),
        kind: SourceTokenKind::Syntax(token.token_kind),
        origin: token.origin,
    }
}
