use ide::FileRange;
use itertools::Itertools;
use serde_json::json;
use utils::text_edit::TextRange;

use crate::{
    global_state::snapshot::GlobalStateSnapshot,
    i18n::keys,
    lsp_ext::{ext::SELECT_FUSESOC_PROJECT_CLIENT_COMMAND, from_proto, to_proto},
};

pub(crate) fn handle_inlay_hint(
    snap: GlobalStateSnapshot,
    params: lsp_types::InlayHintParams,
) -> anyhow::Result<Option<Vec<lsp_types::InlayHint>>> {
    let FileRange { file_id, range } =
        from_proto::file_range(&snap, &params.text_document.uri, params.range)?;

    let line_info = snap.line_info(file_id)?;
    let range = TextRange::new(
        range.start().min(line_info.index.text_len()),
        range.end().min(line_info.index.text_len()),
    );

    let config = snap.config.inlay_hint();
    let res = snap
        .analysis
        .inlay_hint(file_id, range, config)?
        .into_iter()
        .map(|hint| to_proto::inlay_hint(&snap, &line_info, hint))
        .collect_vec();

    Ok(Some(res))
}

pub(crate) fn handle_code_lens(
    snap: GlobalStateSnapshot,
    params: lsp_types::CodeLensParams,
) -> anyhow::Result<Option<Vec<lsp_types::CodeLens>>> {
    let file_id = from_proto::file_id(&snap, &params.text_document.uri)?;
    let line_info = snap.line_info(file_id)?;

    if let Some(lenses) = fusesoc_code_lenses(&snap, file_id, &line_info)? {
        tracing::debug!(lens_count = lenses.len(), "provided FuseSoC code lenses");
        return Ok(Some(lenses));
    }

    let config = snap.config.code_lens();

    let res = snap
        .analysis
        .code_lens(file_id, config)?
        .into_iter()
        .filter_map(|lens| to_proto::code_lens(&snap, &line_info, file_id, lens))
        .collect();

    Ok(Some(res))
}

pub(crate) fn handle_code_lens_resolve(
    snap: GlobalStateSnapshot,
    mut code_lens: lsp_types::CodeLens,
) -> anyhow::Result<lsp_types::CodeLens> {
    let Some(data) = code_lens.data.take() else {
        return Ok(code_lens);
    };

    let (file_id, code_lens_kind) = from_proto::code_lens(&snap, data)?;
    let code_lens_kind = snap.analysis.code_lens_resolve(code_lens_kind)?;

    let line_info = snap.line_info(file_id)?;
    let (command, data) = to_proto::code_lens_kind(&snap, file_id, &line_info, code_lens_kind)?;
    let res = lsp_types::CodeLens { range: code_lens.range, command, data };

    Ok(res)
}

fn fusesoc_code_lenses(
    snap: &GlobalStateSnapshot,
    file_id: vfs::FileId,
    line_info: &utils::lines::LineInfo,
) -> anyhow::Result<Option<Vec<lsp_types::CodeLens>>> {
    let Some(path) = snap.file_path(file_id) else {
        return Ok(None);
    };
    let Some(file_name) = path.file_name() else {
        return Ok(None);
    };
    let text = snap.file_text(file_id)?;

    if path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("core")) {
        return Ok(Some(fusesoc_core_code_lenses(snap, file_id, line_info, &path, &text)?));
    }
    if file_name == "vide.toml" {
        return fusesoc_manifest_code_lenses(snap, line_info, &path, &text);
    }

    Ok(None)
}

fn fusesoc_core_code_lenses(
    snap: &GlobalStateSnapshot,
    file_id: vfs::FileId,
    line_info: &utils::lines::LineInfo,
    core_path: &utils::paths::AbsPathBuf,
    text: &str,
) -> anyhow::Result<Vec<lsp_types::CodeLens>> {
    let core_uri = to_proto::url(snap, file_id)?;
    let workspace_path = core_path
        .as_path()
        .parent()
        .ok_or_else(|| anyhow::anyhow!("FuseSoC core has no workspace parent: {core_path}"))?;
    let workspace_uri = lsp_types::Url::from_file_path(workspace_path).map_err(|()| {
        anyhow::anyhow!("FuseSoC workspace path is not a file URL: {workspace_path:?}")
    })?;
    if !project_model::project_manifest::fusesoc_core_candidates(&workspace_path.to_path_buf())
        .iter()
        .any(|candidate| candidate == core_path)
    {
        return Ok(Vec::new());
    }

    let mut lenses = Vec::new();
    if let Some(range) = line_info.index.range_for_line(0) {
        lenses.push(lsp_types::CodeLens {
            range: to_proto::range(line_info, range),
            command: Some(fusesoc_command(
                snap.config.i18n.text(keys::CODE_LENS_FUSESOC_USE_CORE).to_owned(),
                workspace_uri.clone(),
                Some(core_uri.clone()),
                None,
            )),
            data: None,
        });
    }

    let targets = fusesoc_model::cli::read_core_targets_from_text(core_path, text)?;
    for target in targets {
        let line = target.source_line;
        let Some(range) = line_info.index.range_for_line(line) else {
            continue;
        };
        lenses.push(lsp_types::CodeLens {
            range: to_proto::range(line_info, range),
            command: Some(fusesoc_command(
                snap.config
                    .i18n
                    .format(keys::CODE_LENS_FUSESOC_USE_TARGET, [("target", target.name.clone())]),
                workspace_uri.clone(),
                Some(core_uri.clone()),
                Some(target.name),
            )),
            data: None,
        });
    }

    Ok(lenses)
}

fn fusesoc_manifest_code_lenses(
    snap: &GlobalStateSnapshot,
    line_info: &utils::lines::LineInfo,
    manifest_path: &utils::paths::AbsPathBuf,
    text: &str,
) -> anyhow::Result<Option<Vec<lsp_types::CodeLens>>> {
    let document = text.parse::<toml::Value>()?;
    if document.get("fusesoc").is_none() {
        return Ok(None);
    }
    let workspace_path = manifest_path
        .as_path()
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Vide manifest has no workspace parent: {manifest_path}"))?;
    let workspace_uri = lsp_types::Url::from_file_path(workspace_path).map_err(|()| {
        anyhow::anyhow!("Vide workspace path is not a file URL: {workspace_path:?}")
    })?;
    let lens_line = find_line(text, |line| line.trim() == "[fusesoc]").unwrap_or(0);
    let mut lenses = Vec::new();
    if let Some(range) = line_info.index.range_for_line(lens_line) {
        lenses.push(lsp_types::CodeLens {
            range: to_proto::range(line_info, range),
            command: Some(fusesoc_command(
                snap.config.i18n.text(keys::CODE_LENS_FUSESOC_CONFIGURE_PROJECT).to_owned(),
                workspace_uri.clone(),
                None,
                None,
            )),
            data: None,
        });
    }

    Ok(Some(lenses))
}

fn fusesoc_command(
    title: String,
    workspace_uri: lsp_types::Url,
    core_uri: Option<lsp_types::Url>,
    target: Option<String>,
) -> lsp_types::Command {
    let mut args = serde_json::Map::new();
    args.insert("workspaceUri".to_owned(), json!(workspace_uri));
    if let Some(core_uri) = core_uri {
        args.insert("coreUri".to_owned(), json!(core_uri));
    }
    if let Some(target) = target {
        args.insert("target".to_owned(), json!(target));
    }
    lsp_types::Command {
        title,
        command: SELECT_FUSESOC_PROJECT_CLIENT_COMMAND.to_owned(),
        arguments: Some(vec![serde_json::Value::Object(args)]),
    }
}

fn find_line(text: &str, predicate: impl Fn(&str) -> bool) -> Option<u32> {
    text.lines().enumerate().find_map(|(line, text)| predicate(text).then_some(line as u32))
}

pub(crate) fn handle_signature_help(
    snap: GlobalStateSnapshot,
    params: lsp_types::SignatureHelpParams,
) -> anyhow::Result<Option<lsp_types::SignatureHelp>> {
    let position = from_proto::file_position(&snap, params.text_document_position_params)?;
    let config = snap.config.signature_help();
    let Some(res) = snap.analysis.signature_help(position, config)? else {
        return Ok(None);
    };

    let support_label_offsets = snap.config.cli_signature_help_label_offsets_support();
    let res = to_proto::signature_help(res, support_label_offsets);
    Ok(Some(res))
}
