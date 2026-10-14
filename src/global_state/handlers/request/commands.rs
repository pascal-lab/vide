use serde::de::DeserializeOwned;

use crate::{
    i18n::keys,
    lsp_ext::{
        ext::{
            EXPANDED_RENAME_COMMAND, ExpandedRenameParams, LIST_FUSESOC_TARGETS_COMMAND,
            ListFuseSocTargetsParams, RELOAD_WORKSPACE_COMMAND, RENAME_CONFLICT_INFO_COMMAND,
            RENAME_EXPANSION_INFO_COMMAND, RUN_QIHE_ANALYSIS_COMMAND, RenameConflictInfoParams,
            RenameConflictInfoResult, RenameExpansionInfoParams, RenameExpansionInfoResult,
            RunQiheAnalysisParams, SELECT_FUSESOC_PROJECT_COMMAND, SelectFuseSocProjectParams,
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

fn validate_fusesoc_selection_workspace(
    state: &mut crate::global_state::GlobalState,
    workspace_uri: &lsp_types::Url,
    core_uri: &lsp_types::Url,
) -> anyhow::Result<(utils::paths::AbsPathBuf, utils::paths::AbsPathBuf)> {
    let workspace_root = from_proto::abs_path(workspace_uri)?;
    anyhow::ensure!(
        state.config_state.config.workspace_roots.iter().any(|root| root == &workspace_root),
        "FuseSoC workspace root is not an open workspace: {workspace_root}"
    );
    let core_path = from_proto::abs_path(core_uri)?;
    anyhow::ensure!(
        project_model::project_manifest::fusesoc_core_candidates(&workspace_root)
            .iter()
            .any(|candidate| candidate == &core_path),
        "selected FuseSoC core is not a direct .core candidate in {workspace_root}: {core_path}"
    );
    Ok((workspace_root, core_path))
}

fn handle_select_fusesoc_project_command(
    state: &mut crate::global_state::GlobalState,
    params: lsp_types::ExecuteCommandParams,
) -> anyhow::Result<Option<serde_json::Value>> {
    let params = extract_execute_arg::<SelectFuseSocProjectParams>(state, &params)?;
    let (workspace_root, core_path) =
        validate_fusesoc_selection_workspace(state, &params.workspace_uri, &params.core_uri)?;
    let manifest_path = project_model::project_manifest::persist_fusesoc_selection(
        &workspace_root,
        &core_path,
        params.target.as_deref(),
    )?;

    tracing::info!(
        workspace_root = %workspace_root,
        core_path = %core_path,
        target = ?params.target,
        manifest_path = %manifest_path,
        "persisted FuseSoC project selection"
    );
    let config = triomphe::Arc::make_mut(&mut state.config_state.config);
    config.refresh_project_manifests();
    state.request_workspace_reload("FuseSoC root core selected");
    Ok(None)
}

fn handle_list_fusesoc_targets_command(
    state: &mut crate::global_state::GlobalState,
    params: lsp_types::ExecuteCommandParams,
) -> anyhow::Result<Option<serde_json::Value>> {
    let params = extract_execute_arg::<ListFuseSocTargetsParams>(state, &params)?;
    let (_, core_path) =
        validate_fusesoc_selection_workspace(state, &params.workspace_uri, &params.core_uri)?;
    let targets = fusesoc_model::cli::read_core_targets(&core_path)?;
    Ok(Some(serde_json::to_value(targets)?))
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
        LIST_FUSESOC_TARGETS_COMMAND => handle_list_fusesoc_targets_command(state, params),
        SELECT_FUSESOC_PROJECT_COMMAND => handle_select_fusesoc_project_command(state, params),
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
