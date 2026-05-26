use ide::{
    code_action::{
        CodeActionDiagnostic, CodeActionDiagnostics, CodeActionKind, CodeActionResolveStrategy,
        DiagnosticCode, DiagnosticSource, RepairKind,
    },
    diagnostics as ide_diagnostics,
};
use span::FileRange;
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
        CodeActionResolveStrategy::None
    } else {
        CodeActionResolveStrategy::All
    };

    let line_info = snap.line_info(file_id)?;
    let server_diagnostics = server_diagnostics_for_code_action(
        &snap,
        file_id,
        range,
        &params.context.diagnostics,
        &line_info,
    )?;
    let repair_diagnostics = code_action_diagnostics_from_ide(&server_diagnostics);
    let action =
        snap.analysis.code_action(file_id, range, repair_diagnostics, resolve_strategy.clone())?;

    let mut res = Vec::new();
    for (id, mut assist) in action.into_iter().enumerate() {
        let resolve_data = resolve_strategy
            .is_none()
            .then(|| (id, params.clone(), snap.url_file_version(&params.text_document.uri)));
        let action_diags =
            if let Some(filtered) = quick_fix_diagnostics(assist.id.repair, &server_diagnostics) {
                assist.id.kind = CodeActionKind::QuickFix;
                Some(
                    filtered
                        .into_iter()
                        .map(|diag| to_proto::diagnostic(snap.config.i18n, &line_info, diag))
                        .collect(),
                )
            } else {
                None
            };
        let code_action = to_proto::code_action(&snap, assist, resolve_data, action_diags)?;
        res.push(lsp_types::CodeActionOrCommand::CodeAction(code_action))
    }

    Ok(Some(res))
}

fn quick_fix_diagnostics(
    repair: Option<RepairKind>,
    diagnostics: &[ide_diagnostics::Diagnostic],
) -> Option<Vec<ide_diagnostics::Diagnostic>> {
    let repair = repair?;

    let matches = diagnostics
        .iter()
        .filter(|diag| diagnostic_allows_repair(diag, repair))
        .cloned()
        .collect::<Vec<_>>();
    if matches.is_empty() { None } else { Some(matches) }
}

fn diagnostic_allows_repair(diag: &ide_diagnostics::Diagnostic, repair: RepairKind) -> bool {
    code_action_diagnostic_from_ide(diag).allows_repair(repair)
}

fn server_diagnostics_for_code_action(
    snap: &GlobalStateSnapshot,
    file_id: FileId,
    range: TextRange,
    client_diagnostics: &[lsp_types::Diagnostic],
    line_info: &utils::lines::LineInfo,
) -> anyhow::Result<Vec<ide_diagnostics::Diagnostic>> {
    let server_diagnostics = snap.diagnostics(file_id)?;
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
    fn from_ide(diag: &ide_diagnostics::Diagnostic) -> Self {
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

fn code_action_diagnostics_from_ide(
    diagnostics: &[ide_diagnostics::Diagnostic],
) -> CodeActionDiagnostics {
    CodeActionDiagnostics {
        items: diagnostics.iter().map(code_action_diagnostic_from_ide).collect(),
    }
}

fn code_action_diagnostic_from_ide(diag: &ide_diagnostics::Diagnostic) -> CodeActionDiagnostic {
    CodeActionDiagnostic {
        source: match diag.source {
            ide_diagnostics::DiagnosticSource::SlangParse => Some(DiagnosticSource::Parse),
            ide_diagnostics::DiagnosticSource::SlangSemantic => Some(DiagnosticSource::Semantic),
            ide_diagnostics::DiagnosticSource::Vizsla => None,
        },
        code: Some(DiagnosticCode { subsystem: diag.subsystem, code: diag.code }),
        name: Some(diag.name.clone()),
        option: diag.option_name.clone(),
    }
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

    let (idx, name) = parse_action_id(&data.id).map_err(|err| {
        to_proto::code_action_resolve_error(
            snap.config.i18n,
            CodeActionResolveError::InvalidId(err),
        )
    })?;
    let resolve_strategy = CodeActionResolveStrategy::Single { name };

    let server_diagnostics = server_diagnostics_for_code_action(
        &snap,
        file_id,
        range,
        &data.code_action_params.context.diagnostics,
        &line_index,
    )?;
    let repair_diagnostics = code_action_diagnostics_from_ide(&server_diagnostics);
    let mut actions =
        snap.analysis.code_action(file_id, range, repair_diagnostics, resolve_strategy)?;
    let action = if idx < actions.len() {
        actions.remove(idx)
    } else {
        return Err(to_proto::code_action_resolve_error(
            snap.config.i18n,
            CodeActionResolveError::Stable,
        )
        .into());
    };

    let resolved_action = to_proto::code_action(&snap, action, None, None)?;
    code_action.edit = resolved_action.edit;
    code_action.command = resolved_action.command;

    Ok(code_action)
}

fn parse_action_id(action_id: &str) -> anyhow::Result<(usize, String), String> {
    let id_parts = action_id.split(':').collect::<Vec<_>>();
    match id_parts.as_slice() {
        [assist_name, index] => {
            let index: usize = index.parse().map_err(|_| "Incorrect index string")?;
            Ok((index, assist_name.to_string()))
        }
        _ => Err("Action id contains incorrect number of segments".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use ide::{
        code_action::RepairKind,
        diagnostics::{Diagnostic as IdeDiagnostic, DiagnosticSource as IdeDiagnosticSource},
    };
    use lsp_types::{Diagnostic as LspDiagnostic, NumberOrString, Position, Range};
    use syntax::DiagnosticSeverity;
    use triomphe::Arc;
    use utils::{
        line_index::{LineIndex, TextRange, TextSize},
        lines::{LineEnding, LineInfo, PositionEncoding},
    };
    use vfs::FileId;

    use super::{DiagnosticLocator, code_action_diagnostics_from_ide, quick_fix_diagnostics};

    fn ide_diagnostic(
        name: &str,
        subsystem: u16,
        code: u16,
        option_name: Option<&str>,
    ) -> IdeDiagnostic {
        IdeDiagnostic {
            file_id: FileId(0),
            code,
            subsystem,
            name: name.to_owned(),
            option_name: option_name.map(ToOwned::to_owned),
            groups: Vec::new(),
            source: IdeDiagnosticSource::SlangSemantic,
            range: TextRange::empty(TextSize::from(0)),
            severity: DiagnosticSeverity::Error,
            message: "localized message".to_owned(),
            message_key: None,
            message_args: Vec::new(),
            tags: Vec::new(),
        }
    }

    #[test]
    fn quick_fix_diagnostics_use_server_diagnostic_metadata() {
        let diag = ide_diagnostic("ParamHasNoValue", 2, 29, None);

        let matches = quick_fix_diagnostics(Some(RepairKind::MissingParameter), &[diag]).unwrap();

        assert_eq!(matches[0].name, "ParamHasNoValue");
    }

    #[test]
    fn quick_fix_diagnostics_match_connection_options() {
        let diag = ide_diagnostic("UnconnectedNamedPort", 2, 260, Some("unconnected-port"));

        assert!(quick_fix_diagnostics(Some(RepairKind::MissingConnection), &[diag]).is_some());
    }

    #[test]
    fn quick_fix_diagnostics_match_repair_kinds() {
        let cases = [
            (RepairKind::ConvertOrderedPorts, "MixingOrderedAndNamedPorts"),
            (RepairKind::RemoveEmptyPortConnections, "MixingOrderedAndNamedPorts"),
            (RepairKind::ConvertOrderedParams, "MixingOrderedAndNamedParams"),
            (RepairKind::AddImplicitNamedPortParens, "ImplicitNamedPortNotFound"),
            (RepairKind::AddInstanceParens, "InstanceMissingParens"),
        ];

        for (repair, name) in cases {
            let diag = ide_diagnostic(name, 2, 0, None);
            assert!(quick_fix_diagnostics(Some(repair), &[diag]).is_some());
        }

        let diag = ide_diagnostic("MixingOrderedAndNamedPorts", 2, 0, None);
        assert!(quick_fix_diagnostics(None, &[diag]).is_none());
    }

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
        let ide_diag = IdeDiagnostic {
            file_id: FileId(0),
            code: 129,
            subsystem: 6,
            name: "MixingOrderedAndNamedPorts".to_owned(),
            option_name: None,
            groups: Vec::new(),
            source: IdeDiagnosticSource::SlangSemantic,
            range: TextRange::empty(TextSize::from(6)),
            severity: DiagnosticSeverity::Error,
            message: "mixing ordered and named port connections is not allowed".to_owned(),
            message_key: None,
            message_args: Vec::new(),
            tags: Vec::new(),
        };

        assert_eq!(
            DiagnosticLocator::from_lsp(&line_info, &lsp_diag),
            Some(DiagnosticLocator::from_ide(&ide_diag))
        );
    }

    #[test]
    fn code_action_diagnostics_are_built_from_server_diagnostics() {
        let diag = IdeDiagnostic {
            file_id: FileId(0),
            code: 129,
            subsystem: 6,
            name: "MixingOrderedAndNamedPorts".to_owned(),
            option_name: None,
            groups: Vec::new(),
            source: IdeDiagnosticSource::SlangSemantic,
            range: TextRange::empty(TextSize::from(0)),
            severity: DiagnosticSeverity::Error,
            message: "localized message".to_owned(),
            message_key: None,
            message_args: Vec::new(),
            tags: Vec::new(),
        };

        let diagnostics = code_action_diagnostics_from_ide(&[diag]);

        assert!(diagnostics.allows_repair(RepairKind::ConvertOrderedPorts));
        assert!(diagnostics.allows_repair(RepairKind::RemoveEmptyPortConnections));
    }
}
