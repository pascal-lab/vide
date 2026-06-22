use ide::FileRange;
use utils::text_edit::TextRange;
use vfs::FileId;

use crate::{
    global_state::{response_effect::AcceptedResponseEffect, snapshot::GlobalStateSnapshot},
    lsp::protocol::{from_proto, to_proto},
};

pub(crate) fn handle_semantic_tokens_full(
    snap: GlobalStateSnapshot,
    params: lsp_types::SemanticTokensParams,
) -> anyhow::Result<Option<lsp_types::SemanticTokensResult>> {
    let uri = params.text_document.uri;
    let file_id = from_proto::file_id(&snap, &uri)?;
    let res = compute_sema_tokens_helper(&snap, file_id, None)?;
    snap.cancellation.check()?;
    snap.on_response_accepted(AcceptedResponseEffect::CommitSemanticTokens {
        uri,
        tokens: res.clone(),
    });
    Ok(Some(res.into()))
}

pub(crate) fn handle_semantic_tokens_full_delta(
    snap: GlobalStateSnapshot,
    params: lsp_types::SemanticTokensDeltaParams,
) -> anyhow::Result<Option<lsp_types::SemanticTokensFullDeltaResult>> {
    let uri = params.text_document.uri;
    let file_id = from_proto::file_id(&snap, &uri)?;
    let res = compute_sema_tokens_helper(&snap, file_id, None)?;
    snap.cancellation.check()?;

    let old_tokens = snap.sema_tokens_cache.lock().get(&uri).cloned();
    snap.on_response_accepted(AcceptedResponseEffect::CommitSemanticTokens {
        uri,
        tokens: res.clone(),
    });
    if let Some(old_tokens @ lsp_types::SemanticTokens { result_id: Some(prev_id), .. }) =
        &old_tokens
        && *prev_id == params.previous_result_id
    {
        let delta = to_proto::semantic_token_delta(old_tokens, &res);
        Ok(Some(delta.into()))
    } else {
        Ok(Some(res.into()))
    }
}

pub(crate) fn handle_semantic_tokens_range(
    snap: GlobalStateSnapshot,
    params: lsp_types::SemanticTokensRangeParams,
) -> anyhow::Result<Option<lsp_types::SemanticTokensRangeResult>> {
    let FileRange { file_id, range } =
        from_proto::file_range(&snap, &params.text_document.uri, params.range)?;
    let res = compute_sema_tokens_helper(&snap, file_id, Some(range))?;
    Ok(Some(res.into()))
}

fn compute_sema_tokens_helper(
    snap: &GlobalStateSnapshot,
    file_id: FileId,
    range: Option<TextRange>,
) -> anyhow::Result<lsp_types::SemanticTokens> {
    let text = snap.analysis.file_text(file_id)?;
    let line_info = snap.line_info(file_id)?;
    let config = snap.config.semantic_tokens();
    let tokens = snap.analysis.semantic_tokens(file_id, config, range)?;

    let res = to_proto::semantic_tokens(&text, &line_info, tokens);
    Ok(res)
}
