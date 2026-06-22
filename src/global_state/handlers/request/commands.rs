use serde::de::DeserializeOwned;

use crate::{
    i18n::keys,
    lsp::protocol::{
        ext::{
            EXPANDED_RENAME_COMMAND, ExpandedRenameParams, RELOAD_WORKSPACE_COMMAND,
            RENAME_CONFLICT_INFO_COMMAND, RENAME_EXPANSION_INFO_COMMAND, RUN_QIHE_ANALYSIS_COMMAND,
            RenameConflictInfoParams, RenameConflictInfoResult, RenameExpansionInfoParams,
            RenameExpansionInfoResult, RunQiheAnalysisParams,
        },
        from_proto, to_proto,
    },
};

fn handle_qihe_analysis_command(
    state: &mut crate::global_state::GlobalState,
    params: lsp_types::ExecuteCommandParams,
) -> anyhow::Result<Option<serde_json::Value>> {
    let params = extract_execute_arg::<RunQiheAnalysisParams>(state, &params)?;
    state.spawn_qihe_analysis(params);
    Ok(None)
}

fn handle_reload_workspace_command(
    state: &mut crate::global_state::GlobalState,
) -> anyhow::Result<Option<serde_json::Value>> {
    let config = triomphe::Arc::make_mut(&mut state.config_state.config);
    config.refresh_project_manifests();
    state.request_workspace_reload("workspace reload command");
    Ok(None)
}

fn handle_rename_expansion_info_command(
    state: &mut crate::global_state::GlobalState,
    params: lsp_types::ExecuteCommandParams,
) -> anyhow::Result<Option<serde_json::Value>> {
    let params = extract_execute_arg::<RenameExpansionInfoParams>(state, &params)?;
    let snap = state.make_snapshot();
    let position = from_proto::file_position(&snap, params.text_document_position)?;
    let config = snap.rename_config(position.file_id);
    let info = snap
        .analysis
        .rename_expansion_info(position, config)?
        .map_err(|err| to_proto::rename_error(snap.config.i18n, err))?;
    let result = RenameExpansionInfoResult { additional_symbols: info.additional_symbols };
    Ok(Some(serde_json::to_value(result)?))
}

fn handle_expanded_rename_command(
    state: &mut crate::global_state::GlobalState,
    params: lsp_types::ExecuteCommandParams,
) -> anyhow::Result<Option<serde_json::Value>> {
    let params = extract_execute_arg::<ExpandedRenameParams>(state, &params)?;
    let snap = state.make_snapshot();
    let position = from_proto::file_position(&snap, params.text_document_position)?;
    let config = snap.rename_config(position.file_id);
    let change = snap
        .analysis
        .expanded_rename(position, config, &params.new_name)?
        .map_err(|err| to_proto::rename_error(snap.config.i18n, err))?;
    let workspace_edit = to_proto::workspace_edit(&snap, change)?;
    Ok(Some(serde_json::to_value(workspace_edit)?))
}

fn handle_rename_conflict_info_command(
    state: &mut crate::global_state::GlobalState,
    params: lsp_types::ExecuteCommandParams,
) -> anyhow::Result<Option<serde_json::Value>> {
    let params = extract_execute_arg::<RenameConflictInfoParams>(state, &params)?;
    let snap = state.make_snapshot();
    let position = from_proto::file_position(&snap, params.text_document_position)?;
    let config = snap.rename_config(position.file_id);
    let info = snap
        .analysis
        .rename_conflict_info(position, config, &params.new_name, params.recursive)?
        .map_err(|err| to_proto::rename_error(snap.config.i18n, err))?;
    let result = RenameConflictInfoResult { conflicts: info.conflicts };
    Ok(Some(serde_json::to_value(result)?))
}

fn extract_execute_arg<T: DeserializeOwned>(
    state: &crate::global_state::GlobalState,
    params: &lsp_types::ExecuteCommandParams,
) -> anyhow::Result<T> {
    let args = params.arguments.first().cloned().ok_or_else(|| {
        anyhow::format_err!(
            "{}",
            state.config_state.config.i18n.text(keys::EXECUTE_COMMAND_MISSING_ARGUMENTS)
        )
    })?;
    Ok(serde_json::from_value(args)?)
}

pub(crate) fn handle_execute_command(
    state: &mut crate::global_state::GlobalState,
    params: lsp_types::ExecuteCommandParams,
) -> anyhow::Result<Option<serde_json::Value>> {
    match params.command.as_str() {
        RUN_QIHE_ANALYSIS_COMMAND => handle_qihe_analysis_command(state, params),
        RELOAD_WORKSPACE_COMMAND => handle_reload_workspace_command(state),
        RENAME_EXPANSION_INFO_COMMAND => handle_rename_expansion_info_command(state, params),
        EXPANDED_RENAME_COMMAND => handle_expanded_rename_command(state, params),
        RENAME_CONFLICT_INFO_COMMAND => handle_rename_conflict_info_command(state, params),
        _ => anyhow::bail!(
            "{}",
            state
                .config_state
                .config
                .i18n
                .format(keys::EXECUTE_COMMAND_UNKNOWN, [("command", params.command.clone())])
        ),
    }
}
