use syntax::{
    Trace,
    preproc::{MacroCallId as TraceMacroCallId, TokenOrigin},
};

use super::*;
use crate::{
    macro_file::{
        MacroCallId, MacroCallLoc, Origin, SourceEmittedTokenId, SourceEmittedTokenRange,
    },
    source_db::{PreprocSourceMapping, PreprocVirtualOrigin},
};

pub(in crate::preproc) fn diagnostic_target_for_call(
    db: &dyn PreprocDb,
    model_file: FileId,
    mapped: &MappedSourcePreprocModel,
    source_call: &SourceMacroCall,
) -> PreprocResult<Option<DiagnosticTarget>> {
    let Some(trace_call) = source_call.trace_call else {
        return Ok(None);
    };
    let parsed = db.parsed_compilation_unit(model_file);
    let Some(trace) = parsed.preprocessor_trace.as_ref() else {
        return Ok(None);
    };
    let Some(emitted_range) = db.trace_index(model_file).emitted_range_for_call(trace_call) else {
        return Ok(None);
    };
    diagnostic_target_for_source_expansion(
        db,
        model_file,
        mapped,
        source_call,
        trace,
        emitted_range,
    )
}

enum TokenDiagnosticTarget {
    Target(DiagnosticTarget),
    Skip,
    Blocked,
}

fn diagnostic_target_for_token(
    db: &dyn crate::db::PreprocDb,
    model_file: FileId,
    mapped: &MappedSourcePreprocModel,
    source_call: &SourceMacroCall,
    origin: &TokenOrigin,
) -> PreprocResult<TokenDiagnosticTarget> {
    Ok(match origin {
        TokenOrigin::Source { token_range } => {
            let (source, range) = match source_range_from_trace(token_range) {
                Some(range) => map_source_mapping_range(mapped, range)?,
                None => return Ok(TokenDiagnosticTarget::Skip),
            };
            let file_id = require_file_backed_source(&source)?;
            TokenDiagnosticTarget::Target(DiagnosticTarget {
                origin: Origin::File { file: file_id, range },
                file_id,
                range,
            })
        }
        TokenOrigin::MacroBody { call_id, definition_id, body_token_range, .. } => {
            if source_is_predefine(mapped, body_token_range.buffer_id) {
                return Ok(TokenDiagnosticTarget::Skip);
            }
            let (source, range) = match source_range_from_trace(body_token_range) {
                Some(range) => map_source_mapping_range(mapped, range)?,
                None => return Ok(TokenDiagnosticTarget::Skip),
            };
            let file_id = require_file_backed_source(&source)?;
            TokenDiagnosticTarget::Target(DiagnosticTarget {
                origin: Origin::MacroBody {
                    call: hir_macro_call(db, model_file, *call_id),
                    def: *definition_id,
                    body_range: range,
                },
                file_id,
                range,
            })
        }
        TokenOrigin::MacroArgument { call_id, argument_index, argument_token_range, .. } => {
            let Ok(arg_index) = usize::try_from(*argument_index) else {
                return Ok(TokenDiagnosticTarget::Blocked);
            };
            let (source, range) = match source_range_from_trace(argument_token_range) {
                Some(range) => match map_source_mapping_range(mapped, range) {
                    Ok(mapped) => mapped,
                    Err(error) => {
                        tracing::warn!(
                            ?model_file,
                            ?call_id,
                            ?argument_index,
                            ?error,
                            "macro argument diagnostic target mapping failed"
                        );
                        return Ok(TokenDiagnosticTarget::Blocked);
                    }
                },
                None => return Ok(TokenDiagnosticTarget::Blocked),
            };
            let file_id = require_file_backed_source(&source)?;
            TokenDiagnosticTarget::Target(DiagnosticTarget {
                origin: Origin::MacroArg {
                    call: hir_macro_call(db, model_file, *call_id),
                    arg_index,
                    arg_range: range,
                },
                file_id,
                range,
            })
        }
        TokenOrigin::Predefine { .. } => TokenDiagnosticTarget::Skip,
        TokenOrigin::TokenPaste { .. } => TokenDiagnosticTarget::Blocked,
        TokenOrigin::Stringify { .. } => TokenDiagnosticTarget::Blocked,
        TokenOrigin::Builtin { name, call_id, .. } if !name.is_empty() => {
            let (source, range) = map_source_mapping_range(mapped, source_call.call_range)?;
            let file_id = require_file_backed_source(&source)?;
            TokenDiagnosticTarget::Target(DiagnosticTarget {
                origin: Origin::Builtin {
                    call: hir_macro_call(db, model_file, *call_id),
                    name: SmolStr::new(name),
                },
                file_id,
                range,
            })
        }
        TokenOrigin::Builtin { .. } | TokenOrigin::Unavailable => TokenDiagnosticTarget::Skip,
    })
}

fn source_range_from_trace(range: &syntax::SourceBufferRange) -> Option<SourceRange> {
    Some(SourceRange {
        source: PreprocSourceId::from(range.buffer_id),
        range: TextRange::new(
            TextSize::from(u32::try_from(range.range.start).ok()?),
            TextSize::from(u32::try_from(range.range.end).ok()?),
        ),
    })
}

fn source_is_predefine(mapped: &MappedSourcePreprocModel, buffer_id: u32) -> bool {
    matches!(
        mapped.source_map.get(PreprocSourceId::from(buffer_id)),
        Some(PreprocSourceMapping::VirtualFile {
            origin: PreprocVirtualOrigin::Predefines { .. },
            ..
        })
    )
}

pub(in crate::preproc) fn diagnostic_target_for_source_expansion(
    db: &dyn crate::db::PreprocDb,
    model_file: FileId,
    mapped: &MappedSourcePreprocModel,
    source_call: &SourceMacroCall,
    trace: &Trace,
    emitted_range: SourceEmittedTokenRange,
) -> PreprocResult<Option<DiagnosticTarget>> {
    let start = emitted_range.start.raw();
    let end = start.saturating_add(emitted_range.len);
    for raw in start..end {
        let token_id = SourceEmittedTokenId::new(raw);
        let Some(token) = trace.emitted_tokens.get(raw) else {
            return Err(PreprocError::SourceQuery(SourcePreprocQueryError::MissingEmittedToken {
                token: token_id,
            }));
        };
        match diagnostic_target_for_token(db, model_file, mapped, source_call, &token.origin)? {
            TokenDiagnosticTarget::Target(target) => return Ok(Some(target)),
            TokenDiagnosticTarget::Skip => {}
            TokenDiagnosticTarget::Blocked => {}
        }
    }

    Ok(None)
}

fn hir_macro_call(
    db: &dyn crate::db::PreprocDb,
    model_file: FileId,
    trace_call: TraceMacroCallId,
) -> MacroCallId {
    MacroCallId::new(db, MacroCallLoc { model_file, trace_call })
}
