use ide::FileRange;
use utils::text_edit::TextRange;
use vfs::FileId;

use crate::{
    global_state::snapshot::GlobalStateSnapshot,
    lsp_ext::{ext::CodeActionResolveError, from_proto, to_proto},
};

pub(crate) fn handle_code_action(
    snap: GlobalStateSnapshot,
    params: lsp_types::CodeActionParams,
) -> anyhow::Result<Option<Vec<lsp_types::CodeActionOrCommand>>> {
    if !snap.config.cli_code_action_literals() {
        return Ok(None);
    }

    let FileRange { file_id, range } =
        from_proto::file_range(&snap, &params.text_document.uri, params.range)?;

    let resolve_strategy = if snap.config.cli_code_action_resolve() {
        ide::code_action::CodeActionResolveStrategy::None
    } else {
        ide::code_action::CodeActionResolveStrategy::All
    };

    let line_info = snap.line_info(file_id)?;
    let server_diagnostics = server_diagnostics_for_code_action(
        &snap,
        file_id,
        range,
        &params.context.diagnostics,
        &line_info,
    )?;
    let actions =
        snap.analysis.code_action(file_id, range, &server_diagnostics, resolve_strategy.clone())?;

    let mut res = Vec::new();
    for action in actions {
        let resolve_data = resolve_strategy
            .is_none()
            .then(|| (params.clone(), snap.url_file_version(&params.text_document.uri)));
        let action_diags = if action.diagnostics.is_empty() {
            None
        } else {
            Some(
                action
                    .diagnostics
                    .iter()
                    .map(|diag| to_proto::diagnostic(snap.config.i18n, &line_info, diag.clone()))
                    .collect(),
            )
        };
        let code_action = to_proto::code_action(&snap, action, resolve_data, action_diags)?;
        res.push(lsp_types::CodeActionOrCommand::CodeAction(code_action))
    }

    Ok(Some(res))
}

fn server_diagnostics_for_code_action(
    snap: &GlobalStateSnapshot,
    file_id: FileId,
    range: TextRange,
    client_diagnostics: &[lsp_types::Diagnostic],
    line_info: &utils::lines::LineInfo,
) -> anyhow::Result<Vec<ide::diagnostics::Diagnostic>> {
    let mut server_diagnostics = snap.diagnostics(file_id)?;
    server_diagnostics.extend(snap.external_diagnostics(file_id));
    let client_locators = client_diagnostics
        .iter()
        .filter_map(|diag| DiagnosticLocator::from_lsp(line_info, diag))
        .collect::<Vec<_>>();

    let diagnostics = if client_locators.is_empty() {
        server_diagnostics
            .into_iter()
            .filter(|diag| diagnostic_range_matches_request(diag.range, range))
            .collect()
    } else {
        server_diagnostics
            .into_iter()
            .filter(|diag| {
                let locator = DiagnosticLocator::from_ide(diag);
                client_locators.iter().any(|client| client == &locator)
            })
            .collect()
    };

    Ok(diagnostics)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiagnosticLocator {
    range: TextRange,
    code: String,
}

impl DiagnosticLocator {
    fn from_ide(diag: &ide::diagnostics::Diagnostic) -> Self {
        Self { range: diag.range, code: format!("{}:{}", diag.subsystem, diag.code) }
    }

    fn from_lsp(line_info: &utils::lines::LineInfo, diag: &lsp_types::Diagnostic) -> Option<Self> {
        if diag.source.as_deref() != Some("slang") {
            return None;
        }

        Some(Self {
            range: from_proto::text_range(line_info, diag.range).ok()?,
            code: diagnostic_code_string(diag.code.as_ref()?),
        })
    }
}

fn diagnostic_code_string(code: &lsp_types::NumberOrString) -> String {
    match code {
        lsp_types::NumberOrString::Number(code) => code.to_string(),
        lsp_types::NumberOrString::String(code) => code.clone(),
    }
}

fn diagnostic_range_matches_request(diag: TextRange, request: TextRange) -> bool {
    if request.is_empty() {
        let offset = request.start();
        return if diag.is_empty() {
            diag.start() == offset
        } else {
            diag.start() <= offset && offset <= diag.end()
        };
    }

    if diag.is_empty() {
        let offset = diag.start();
        return request.start() <= offset && offset <= request.end();
    }

    diag.start() < request.end() && request.start() < diag.end()
}

pub(crate) fn handle_code_action_resolve(
    snap: GlobalStateSnapshot,
    mut code_action: lsp_types::CodeAction,
) -> anyhow::Result<lsp_types::CodeAction> {
    let data = from_proto::code_action_data(
        code_action.data.replace(Default::default()).ok_or_else(|| {
            to_proto::code_action_resolve_error(snap.config.i18n, CodeActionResolveError::NoData)
        })?,
    )?;

    let file_id = from_proto::file_id(&snap, &data.code_action_params.text_document.uri)?;
    if snap.url_file_version(&data.code_action_params.text_document.uri) != data.version {
        return Err(to_proto::code_action_resolve_error(
            snap.config.i18n,
            CodeActionResolveError::Stable,
        )
        .into());
    }

    let line_index = snap.line_info(file_id)?;
    let range = from_proto::text_range(&line_index, data.code_action_params.range)?;

    let resolve_strategy =
        ide::code_action::CodeActionResolveStrategy::Single { name: data.id.clone() };

    let server_diagnostics = server_diagnostics_for_code_action(
        &snap,
        file_id,
        range,
        &data.code_action_params.context.diagnostics,
        &line_index,
    )?;
    let actions =
        snap.analysis.code_action(file_id, range, &server_diagnostics, resolve_strategy)?;
    let action = actions.into_iter().find(|action| action.id.name == data.id).ok_or_else(|| {
        to_proto::code_action_resolve_error(snap.config.i18n, CodeActionResolveError::Stable)
    })?;

    let resolved_action = to_proto::code_action(&snap, action, None, None)?;
    code_action.edit = resolved_action.edit;
    code_action.command = resolved_action.command;

    Ok(code_action)
}

#[cfg(test)]
mod tests {
    use lsp_types::{Diagnostic as LspDiagnostic, NumberOrString, Position, Range};
    use syntax::DiagnosticSeverity;
    use triomphe::Arc;
    use utils::{
        line_index::{LineIndex, TextRange, TextSize},
        lines::{LineEnding, LineInfo, PositionEncoding},
    };
    use vfs::FileId;

    use super::DiagnosticLocator;

    #[test]
    fn diagnostic_locator_matches_client_diagnostic_without_data() {
        let line_info = LineInfo {
            index: Arc::new(LineIndex::new("module top;\nendmodule\n")),
            ending: LineEnding::Unix,
            encoding: PositionEncoding::Utf8,
        };
        let range = Range::new(Position::new(0, 6), Position::new(0, 6));
        let lsp_diag = LspDiagnostic {
            range,
            severity: None,
            code: Some(NumberOrString::String("6:129".to_owned())),
            code_description: None,
            source: Some("slang".to_owned()),
            message: "mixing ordered and named port connections is not allowed".to_owned(),
            related_information: None,
            tags: None,
            data: None,
        };
        let ide_diag = ide::diagnostics::Diagnostic {
            file_id: FileId::from_raw(0),
            code: 129,
            subsystem: 6,
            name: "MixingOrderedAndNamedPorts".to_owned(),
            option_name: None,
            groups: Vec::new(),
            source: ide::diagnostics::DiagnosticSource::SlangSemantic,
            range: TextRange::empty(TextSize::from(6)),
            severity: DiagnosticSeverity::Error,
            message: "mixing ordered and named port connections is not allowed".to_owned(),
            args: Vec::new(),
            message_key: None,
            message_args: Vec::new(),
            tags: Vec::new(),
        };

        assert_eq!(
            DiagnosticLocator::from_lsp(&line_info, &lsp_diag),
            Some(DiagnosticLocator::from_ide(&ide_diag))
        );
    }
}
