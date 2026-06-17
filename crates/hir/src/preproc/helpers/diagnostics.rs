use super::*;
use crate::hir_def::macro_file::Origin;

pub(in crate::preproc) fn diagnostic_target_for_call(
    mapped: &MappedSourcePreprocModel,
    source_call: &SourceMacroCall,
) -> PreprocResult<Option<DiagnosticTarget>> {
    match mapped.model.immediate_macro_expansion(source_call.id) {
        SourceMacroExpansionQuery::Available(expansion_id) => {
            let Some(expansion) = mapped.model.macro_expansions().get(expansion_id) else {
                return Ok(None);
            };
            diagnostic_target_for_source_expansion(mapped, expansion)
        }
        SourceMacroExpansionQuery::Unavailable(_) => Ok(None),
    }
}

pub(in crate::preproc) fn emitted_token_ids(
    range: SourceEmittedTokenRange,
) -> impl Iterator<Item = SourceEmittedTokenId> {
    let start = range.start.raw();
    let end = start.saturating_add(range.len);
    (start..end).map(SourceEmittedTokenId::new)
}

enum TokenDiagnosticTarget {
    Target(DiagnosticTarget),
    Skip,
    Blocked,
}

fn diagnostic_target_for_token(
    mapped: &MappedSourcePreprocModel,
    origin: &SourceTokenOrigin,
) -> PreprocResult<TokenDiagnosticTarget> {
    Ok(match origin {
        SourceTokenOrigin::Source { token_range } => {
            let (source, range) = map_source_mapping_range(mapped, *token_range)?;
            let file_id = require_file_backed_source(&source)?;
            TokenDiagnosticTarget::Target(DiagnosticTarget {
                origin: Origin::File { file: file_id, range },
                file_id,
                range,
            })
        }
        SourceTokenOrigin::MacroBody { origin, body_token_range, call, .. } => {
            let _call = mapped_macro_call(mapped, *call)?;
            let (source, range) = map_source_mapping_range(mapped, *body_token_range)?;
            let file_id = require_file_backed_source(&source)?;
            TokenDiagnosticTarget::Target(DiagnosticTarget {
                origin: Origin::MacroBody {
                    call: origin.call_id,
                    def: origin.definition_id,
                    body_range: range,
                },
                file_id,
                range,
            })
        }
        SourceTokenOrigin::MacroArgument {
            origin,
            call,
            argument_index,
            argument_token_range,
            ..
        } => {
            let _call = mapped_macro_call(mapped, *call)?;
            let Ok((source, range)) = map_source_mapping_range(mapped, *argument_token_range)
            else {
                return Ok(TokenDiagnosticTarget::Blocked);
            };
            let file_id = require_file_backed_source(&source)?;
            TokenDiagnosticTarget::Target(DiagnosticTarget {
                origin: Origin::MacroArg {
                    call: origin.call_id,
                    arg_index: *argument_index,
                    arg_range: range,
                },
                file_id,
                range,
            })
        }
        SourceTokenOrigin::TokenPaste { call, .. } => {
            let _call = mapped_macro_call(mapped, *call)?;
            TokenDiagnosticTarget::Blocked
        }
        SourceTokenOrigin::Stringification { call, .. } => {
            let _call = mapped_macro_call(mapped, *call)?;
            TokenDiagnosticTarget::Blocked
        }
        SourceTokenOrigin::Predefine { source } => {
            let _source = map_source_mapping_id(mapped, *source)?;
            TokenDiagnosticTarget::Skip
        }
        SourceTokenOrigin::Builtin { name, origin, call, .. } => {
            let call = mapped_macro_call(mapped, *call)?;
            TokenDiagnosticTarget::Target(DiagnosticTarget {
                origin: Origin::Builtin { call: origin.call_id, name: name.clone() },
                file_id: call.file_id,
                range: call.range,
            })
        }
    })
}

pub(in crate::preproc) fn mapped_macro_call(
    mapped: &MappedSourcePreprocModel,
    call: SourceMacroCallId,
) -> PreprocResult<MacroCall> {
    let Some(call) = mapped.model.macro_calls().get(call) else {
        return Err(unavailable_error(SourcePreprocUnavailable::MissingMacroCall { call }));
    };
    map_macro_call(mapped, call)
}

pub(in crate::preproc) fn diagnostic_target_for_source_expansion(
    mapped: &MappedSourcePreprocModel,
    expansion: &SourceMacroExpansion,
) -> PreprocResult<Option<DiagnosticTarget>> {
    for token_id in emitted_token_ids(expansion.emitted_token_range) {
        let Some(token) = mapped.model.emitted_tokens().get(token_id) else {
            return Err(PreprocError::SourceMap(PreprocSourceMapError::MissingEmittedToken {
                token: token_id,
            }));
        };
        let Some(origin) = token.origin.and_then(|id| mapped.model.token_origins().get(id)) else {
            continue;
        };
        match diagnostic_target_for_token(mapped, origin)? {
            TokenDiagnosticTarget::Target(target) => return Ok(Some(target)),
            TokenDiagnosticTarget::Skip => {}
            TokenDiagnosticTarget::Blocked => {}
        }
    }

    Ok(None)
}
