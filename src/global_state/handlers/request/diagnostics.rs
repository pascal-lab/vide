use std::collections::{HashMap, HashSet};

use crate::{
    global_state::snapshot::GlobalStateSnapshot,
    lsp_ext::{from_proto, to_proto},
};

pub(crate) fn handle_document_diagnostic(
    snap: GlobalStateSnapshot,
    params: lsp_types::DocumentDiagnosticParams,
) -> anyhow::Result<lsp_types::DocumentDiagnosticReportResult> {
    let file_id = from_proto::file_id(&snap, &params.text_document.uri)?;
    let result_id = snap.document_diagnostic_result_id(file_id, &params.text_document.uri);
    let items = snap.lsp_diagnostics(file_id)?;
    Ok(document_diagnostic_report(result_id, items, params.previous_result_id.as_deref()).into())
}

pub(crate) fn handle_workspace_diagnostic(
    snap: GlobalStateSnapshot,
    params: lsp_types::WorkspaceDiagnosticParams,
) -> anyhow::Result<lsp_types::WorkspaceDiagnosticReportResult> {
    let previous_result_ids = params
        .previous_result_ids
        .into_iter()
        .map(|prev| {
            let original_uri = prev.uri;
            let uri = match from_proto::abs_path(&original_uri)
                .and_then(|path| to_proto::url_from_abs_path(path.as_ref()))
            {
                Ok(uri) => uri,
                Err(error) => {
                    tracing::debug!(
                        uri = %original_uri,
                        "keeping previous diagnostic URI as-is: {error:#}"
                    );
                    original_uri
                }
            };
            (uri, prev.value)
        })
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    let mut diagnostics_by_file = HashMap::new();

    let diagnostic_file_ids = snap.workspace_diagnostic_file_ids();

    for producer in snap.workspace_diagnostic_producers(&diagnostic_file_ids) {
        for diag in snap.workspace_diagnostics_for_producer(&producer)? {
            diagnostics_by_file.entry(diag.file_id).or_insert_with(Vec::new).push(diag);
        }
    }

    for file_id in diagnostic_file_ids {
        let targets = match snap.workspace_diagnostic_targets(file_id) {
            Ok(targets) => targets,
            Err(error) => {
                tracing::debug!(?file_id, "skipping diagnostics for file without URI: {error:#}");
                continue;
            }
        };

        let diagnostics = diagnostics_by_file.remove(&file_id).unwrap_or_default();

        let line_info = snap.line_info(file_id)?;
        let mut diag_items = diagnostics
            .into_iter()
            .map(|diag| to_proto::diagnostic(snap.config.i18n, &line_info, diag))
            .collect::<Vec<_>>();
        diag_items.extend(snap.external_lsp_diagnostics(file_id)?);

        for target in targets {
            let uri = target.uri().clone();
            seen.insert(uri.clone());
            let result_id = snap.workspace_diagnostic_result_id(file_id, &uri);
            let version = target.version().map(|version| version as i64);
            let previous_result_id = previous_result_ids.get(&uri).map(String::as_str);

            items.push(workspace_diagnostic_report(
                uri,
                version,
                result_id,
                diag_items.clone(),
                previous_result_id,
            ));
        }
    }

    for (uri, _) in previous_result_ids {
        if seen.contains(&uri) {
            continue;
        }

        items.push(workspace_diagnostic_report(uri, None, None, Vec::new(), None));
    }

    Ok(lsp_types::WorkspaceDiagnosticReportResult::Report(lsp_types::WorkspaceDiagnosticReport {
        items,
    }))
}

fn document_diagnostic_report(
    result_id: Option<String>,
    items: Vec<lsp_types::Diagnostic>,
    previous_result_id: Option<&str>,
) -> lsp_types::DocumentDiagnosticReport {
    if let Some(result_id) = result_id.as_ref()
        && Some(result_id.as_str()) == previous_result_id
    {
        return lsp_types::DocumentDiagnosticReport::Unchanged(
            lsp_types::RelatedUnchangedDocumentDiagnosticReport {
                related_documents: None,
                unchanged_document_diagnostic_report:
                    lsp_types::UnchangedDocumentDiagnosticReport { result_id: result_id.clone() },
            },
        );
    }

    lsp_types::DocumentDiagnosticReport::Full(lsp_types::RelatedFullDocumentDiagnosticReport {
        related_documents: None,
        full_document_diagnostic_report: lsp_types::FullDocumentDiagnosticReport {
            result_id: result_id.clone(),
            items,
        },
    })
}

fn workspace_diagnostic_report(
    uri: lsp_types::Url,
    version: Option<i64>,
    result_id: Option<String>,
    items: Vec<lsp_types::Diagnostic>,
    previous_result_id: Option<&str>,
) -> lsp_types::WorkspaceDocumentDiagnosticReport {
    if let Some(result_id) = result_id.as_ref()
        && Some(result_id.as_str()) == previous_result_id
    {
        return lsp_types::WorkspaceDocumentDiagnosticReport::Unchanged(
            lsp_types::WorkspaceUnchangedDocumentDiagnosticReport {
                uri,
                version,
                unchanged_document_diagnostic_report:
                    lsp_types::UnchangedDocumentDiagnosticReport { result_id: result_id.clone() },
            },
        );
    }

    lsp_types::WorkspaceDocumentDiagnosticReport::Full(
        lsp_types::WorkspaceFullDocumentDiagnosticReport {
            uri,
            version,
            full_document_diagnostic_report: lsp_types::FullDocumentDiagnosticReport {
                result_id: result_id.clone(),
                items,
            },
        },
    )
}

#[cfg(test)]
mod tests {
    use lsp_types::{
        DocumentDiagnosticReport, UnchangedDocumentDiagnosticReport, Url,
        WorkspaceDocumentDiagnosticReport,
    };

    use super::{document_diagnostic_report, workspace_diagnostic_report};

    #[test]
    fn workspace_diagnostic_report_uses_full_for_new_result_id() {
        let uri = Url::parse("file:///tmp/test.sv").unwrap();
        let report = workspace_diagnostic_report(
            uri.clone(),
            Some(3),
            Some("4".to_string()),
            Vec::new(),
            Some("2"),
        );

        match report {
            WorkspaceDocumentDiagnosticReport::Full(report) => {
                assert_eq!(report.uri, uri);
                assert_eq!(report.version, Some(3));
                assert_eq!(report.full_document_diagnostic_report.result_id.as_deref(), Some("4"));
                assert!(report.full_document_diagnostic_report.items.is_empty());
            }
            other => panic!("expected full report, got {other:?}"),
        }
    }

    #[test]
    fn workspace_diagnostic_report_uses_unchanged_for_matching_result_id() {
        let uri = Url::parse("file:///tmp/test.sv").unwrap();
        let report = workspace_diagnostic_report(
            uri.clone(),
            Some(5),
            Some("5".to_string()),
            Vec::new(),
            Some("5"),
        );

        match report {
            WorkspaceDocumentDiagnosticReport::Unchanged(report) => {
                assert_eq!(report.uri, uri);
                assert_eq!(report.version, Some(5));
                assert_eq!(report.unchanged_document_diagnostic_report.result_id, "5");
            }
            other => panic!("expected unchanged report, got {other:?}"),
        }
    }
    #[test]
    fn document_diagnostic_report_uses_unchanged_for_matching_result_id() {
        let report = document_diagnostic_report(Some("7".to_string()), Vec::new(), Some("7"));

        match report {
            DocumentDiagnosticReport::Unchanged(report) => assert_eq!(
                report.unchanged_document_diagnostic_report,
                UnchangedDocumentDiagnosticReport { result_id: "7".to_string() }
            ),
            other => panic!("expected unchanged report, got {other:?}"),
        }
    }
}
