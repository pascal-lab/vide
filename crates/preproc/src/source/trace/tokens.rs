use syntax::preproc::{
    MacroArgumentOrigin, MacroBodyOrigin, MacroBuiltinOrigin, MacroOperationOrigin,
};

use super::{helpers::source_range_from_trace, *};

pub(super) fn emitted_token_from_trace(token: EmittedToken) -> SourceEmittedTokenFact {
    SourceEmittedTokenFact {
        raw: token.raw_text.to_smolstr(),
        value: token.value_text.to_smolstr(),
        display: token.display_text.to_smolstr(),
        kind: SourceTokenKind::Syntax(token.token_kind),
        provenance: emitted_token_provenance_from_trace(token.provenance),
    }
}

fn emitted_token_provenance_from_trace(provenance: TokenOrigin) -> SourceTokenProvenanceFact {
    match provenance {
        TokenOrigin::Source { token_range } => source_range_from_trace(&token_range)
            .map(|token_range| SourceTokenProvenanceFact::Source { token_range })
            .unwrap_or(SourceTokenProvenanceFact::Unavailable),
        TokenOrigin::MacroBody { macro_name, identity, call_range, body_token_range } => {
            let Some(call_range) = source_range_from_trace(&call_range) else {
                return SourceTokenProvenanceFact::Unavailable;
            };
            let Some(body_token_range) = source_range_from_trace(&body_token_range) else {
                return SourceTokenProvenanceFact::Unavailable;
            };
            SourceTokenProvenanceFact::MacroBody {
                macro_name: macro_name.to_smolstr(),
                identity: Some(macro_body_identity(identity)),
                call_range,
                body_token_range,
            }
        }
        TokenOrigin::MacroArgument {
            macro_name,
            identity,
            call_range,
            body_token_range,
            argument_token_range,
        } => {
            let Some(call_range) = source_range_from_trace(&call_range) else {
                return SourceTokenProvenanceFact::Unavailable;
            };
            let Some(body_token_range) = source_range_from_trace(&body_token_range) else {
                return SourceTokenProvenanceFact::Unavailable;
            };
            let Some(argument_token_range) = source_range_from_trace(&argument_token_range) else {
                return SourceTokenProvenanceFact::Unavailable;
            };
            SourceTokenProvenanceFact::MacroArgument {
                macro_name: macro_name.to_smolstr(),
                identity: Some(macro_argument_identity(identity)),
                call_range,
                body_token_range,
                argument_token_range,
            }
        }
        TokenOrigin::Builtin { name, identity } if !name.is_empty() => {
            SourceTokenProvenanceFact::Builtin {
                name: name.to_smolstr(),
                identity: Some(macro_builtin_identity(identity)),
            }
        }
        TokenOrigin::TokenPaste { identity } => SourceTokenProvenanceFact::TokenPaste {
            identity: Some(macro_operation_identity(identity)),
        },
        TokenOrigin::Stringification { identity } => SourceTokenProvenanceFact::Stringification {
            identity: Some(macro_operation_identity(identity)),
        },
        TokenOrigin::Builtin { .. } => SourceTokenProvenanceFact::Unavailable,
        TokenOrigin::Unavailable => SourceTokenProvenanceFact::Unavailable,
    }
}

fn macro_body_identity(value: MacroBodyOrigin) -> SourceMacroBodyIdentity {
    SourceMacroBodyIdentity {
        call: value.call_id,
        definition: value.definition_id,
        expansion: value.expansion_id,
        parent_expansion: value.parent_expansion_id,
        body_token_index: value.body_token_index as usize,
    }
}

fn macro_argument_identity(value: MacroArgumentOrigin) -> SourceMacroArgumentIdentity {
    SourceMacroArgumentIdentity {
        call: value.call_id,
        definition: value.definition_id,
        expansion: value.expansion_id,
        parent_expansion: value.parent_expansion_id,
        body_token_index: value.body_token_index as usize,
        argument_index: value.argument_index as usize,
        argument_token_index: value.argument_token_index as usize,
    }
}

fn macro_builtin_identity(value: MacroBuiltinOrigin) -> SourceMacroBuiltinIdentity {
    SourceMacroBuiltinIdentity {
        call: value.call_id,
        expansion: value.expansion_id,
        parent_expansion: value.parent_expansion_id,
    }
}

fn macro_operation_identity(value: MacroOperationOrigin) -> SourceMacroOperationIdentity {
    SourceMacroOperationIdentity {
        call: value.call_id,
        definition: value.definition_id,
        expansion: value.expansion_id,
        parent_expansion: value.parent_expansion_id,
        body_token_index: value.body_token_index as usize,
        argument_index: value.argument_index.map(|index| index as usize),
        argument_token_index: value.argument_token_index.map(|index| index as usize),
    }
}
