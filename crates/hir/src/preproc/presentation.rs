use super::*;
use crate::source_map::SourcePresentationAnchor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePresentationFileRange {
    pub file_id: FileId,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcePresentationResolution {
    Available(SourcePresentationFileRange),
    Unavailable(PreprocUnavailable),
    Ambiguous(Vec<SourcePresentationFileRange>),
}

impl SourcePresentationResolution {
    pub fn available(self) -> Option<SourcePresentationFileRange> {
        match self {
            Self::Available(target) => Some(target),
            Self::Unavailable(_) | Self::Ambiguous(_) => None,
        }
    }
}

pub fn resolve_source_presentation_anchor(
    db: &dyn SourceRootDb,
    file_id: FileId,
    anchor: SourcePresentationAnchor,
) -> PreprocResult<SourcePresentationResolution> {
    match anchor {
        SourcePresentationAnchor::Direct(range) => {
            Ok(SourcePresentationResolution::Available(SourcePresentationFileRange {
                file_id,
                range,
            }))
        }
        SourcePresentationAnchor::Source(source_range) => {
            resolve_source_range_anchor(db, file_id, source_range)
        }
        SourcePresentationAnchor::MacroBody(_)
        | SourcePresentationAnchor::MacroArgument(_)
        | SourcePresentationAnchor::MacroOperation(_) => {
            resolve_macro_token_anchor(db, file_id, anchor)
        }
        SourcePresentationAnchor::Unavailable => {
            Ok(SourcePresentationResolution::Unavailable(unavailable_presentation_anchor()))
        }
    }
}

fn resolve_source_range_anchor(
    db: &dyn SourceRootDb,
    file_id: FileId,
    source_range: SourceRange,
) -> PreprocResult<SourcePresentationResolution> {
    let mut targets = Vec::new();
    let mut first_error = None;
    let mut first_unavailable = None;
    let contexts = source_preproc_single_query_contexts(db, file_id);

    for model_file_id in contexts.model_file_ids.iter().copied() {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };
        match map_mapped_source_range(mapped, source_range) {
            Ok((source, range)) => {
                push_original_target_or_unavailable(
                    &mut targets,
                    &mut first_unavailable,
                    source,
                    range,
                );
            }
            Err(error) => record_first_error(&mut first_error, error),
        }
    }

    finish_presentation_targets(targets, &contexts, first_error, first_unavailable)
}

fn resolve_macro_token_anchor(
    db: &dyn SourceRootDb,
    file_id: FileId,
    anchor: SourcePresentationAnchor,
) -> PreprocResult<SourcePresentationResolution> {
    let mut targets = Vec::new();
    let mut first_error = None;
    let mut first_unavailable = None;
    let contexts = source_preproc_single_query_contexts(db, file_id);

    for model_file_id in contexts.model_file_ids.iter().copied() {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };
        collect_macro_token_anchor_targets(
            mapped,
            anchor,
            &mut targets,
            &mut first_error,
            &mut first_unavailable,
        );
    }

    finish_presentation_targets(targets, &contexts, first_error, first_unavailable)
}

fn collect_macro_token_anchor_targets(
    mapped: &MappedSourcePreprocModel,
    anchor: SourcePresentationAnchor,
    targets: &mut Vec<SourcePresentationFileRange>,
    first_error: &mut Option<PreprocError>,
    first_unavailable: &mut Option<PreprocUnavailable>,
) {
    for provenance in mapped.model.token_provenance().iter() {
        if !token_provenance_matches_anchor(provenance, anchor) {
            continue;
        }
        let provenance = match map_token_provenance(mapped, provenance) {
            Ok(provenance) => provenance,
            Err(error) => {
                record_first_error(first_error, error);
                continue;
            }
        };
        let Some((source, range)) = presentation_source_for_token_provenance(&provenance) else {
            record_first_unavailable(first_unavailable, unavailable_presentation_anchor());
            continue;
        };
        push_original_target_or_unavailable(targets, first_unavailable, source, range);
    }
}

fn token_provenance_matches_anchor(
    provenance: &SourceTokenProvenanceFact,
    anchor: SourcePresentationAnchor,
) -> bool {
    match (anchor, provenance) {
        (
            SourcePresentationAnchor::MacroBody(anchor),
            SourceTokenProvenanceFact::MacroBody { identity, .. },
        ) => MacroBodyTokenIdentity::from(*identity) == anchor,
        (
            SourcePresentationAnchor::MacroArgument(anchor),
            SourceTokenProvenanceFact::MacroArgument { identity, .. },
        ) => MacroArgumentTokenIdentity::from(*identity) == anchor,
        (
            SourcePresentationAnchor::MacroOperation(anchor),
            SourceTokenProvenanceFact::TokenPaste { identity, .. }
            | SourceTokenProvenanceFact::Stringification { identity, .. },
        ) => MacroOperationTokenIdentity::from(*identity) == anchor,
        _ => false,
    }
}

fn presentation_source_for_token_provenance(
    provenance: &TokenProvenance,
) -> Option<(MappedPreprocSource, TextRange)> {
    match provenance {
        TokenProvenance::SourceToken { source, range }
        | TokenProvenance::MacroBody { source, range, .. }
        | TokenProvenance::MacroArgument { source, range, .. } => Some((source.clone(), *range)),
        TokenProvenance::TokenPaste { identity, call }
        | TokenProvenance::Stringification { identity, call } => {
            macro_operation_source_token(call, *identity)
        }
        TokenProvenance::Predefine { .. }
        | TokenProvenance::Builtin { .. }
        | TokenProvenance::Unavailable(_) => None,
    }
}

fn macro_operation_source_token(
    call: &MacroCall,
    identity: MacroOperationTokenIdentity,
) -> Option<(MappedPreprocSource, TextRange)> {
    let argument_index = identity.argument_index?;
    let argument_token_index = identity.argument_token_index?;
    let argument = call.arguments.get(argument_index)?;
    if argument.argument_index != argument_index {
        return None;
    }
    let token = argument.tokens.get(argument_token_index)?;
    Some((token.source.as_ref()?.clone(), token.range?))
}

fn push_original_target_or_unavailable(
    targets: &mut Vec<SourcePresentationFileRange>,
    first_unavailable: &mut Option<PreprocUnavailable>,
    source: MappedPreprocSource,
    range: TextRange,
) {
    match source {
        MappedPreprocSource::RealFile { file_id } => {
            push_unique_target(targets, SourcePresentationFileRange { file_id, range });
        }
        MappedPreprocSource::VirtualFile { path, origin, .. }
        | MappedPreprocSource::VirtualDisplay { path, origin } => {
            record_first_unavailable(
                first_unavailable,
                PreprocUnavailable::DisplayOnlyVirtualExpansion { path, origin },
            );
        }
    }
}

fn push_unique_target(
    targets: &mut Vec<SourcePresentationFileRange>,
    target: SourcePresentationFileRange,
) {
    if !targets.contains(&target) {
        targets.push(target);
    }
}

fn finish_presentation_targets(
    targets: Vec<SourcePresentationFileRange>,
    contexts: &SourcePreprocQueryContexts,
    first_error: Option<PreprocError>,
    first_unavailable: Option<PreprocUnavailable>,
) -> PreprocResult<SourcePresentationResolution> {
    match targets.len() {
        0 => {}
        1 => {
            return Ok(SourcePresentationResolution::Available(
                targets.into_iter().next().unwrap(),
            ));
        }
        _ => return Ok(SourcePresentationResolution::Ambiguous(targets)),
    }

    if let Some(reason) = first_unavailable {
        return Ok(SourcePresentationResolution::Unavailable(reason));
    }
    finish_empty_single_query(contexts, first_error)?;
    Ok(SourcePresentationResolution::Unavailable(unavailable_presentation_anchor()))
}

fn record_first_unavailable(
    first_unavailable: &mut Option<PreprocUnavailable>,
    reason: PreprocUnavailable,
) {
    if first_unavailable.is_none() {
        *first_unavailable = Some(reason);
    }
}

fn unavailable_presentation_anchor() -> PreprocUnavailable {
    PreprocUnavailable::Source(SourcePreprocUnavailable::TokenProvenanceAuthorityUnavailable)
}
