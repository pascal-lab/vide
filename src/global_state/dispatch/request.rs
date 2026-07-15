use lsp_server::Request;
use lsp_types::{
    DocumentDiagnosticReport, DocumentDiagnosticReportResult, FullDocumentDiagnosticReport,
    RelatedFullDocumentDiagnosticReport, WorkspaceDiagnosticReport,
    WorkspaceDiagnosticReportResult, WorkspaceSymbolResponse,
};

use crate::{
    global_state::{GlobalState, dispatcher::ReqDispatcher, handlers},
    i18n::keys,
};

impl GlobalState {
    pub(in crate::global_state) fn handle_request(&mut self, mut req: Request) {
        if !self.is_workspace_ready() {
            tracing::debug!(
                method = %req.method,
                id = ?req.id,
                readiness = ?self.workspace.workspace_vfs,
                "workspace is not ready; checking for a terminal fallback response"
            );

            let mut readiness_dispatcher = ReqDispatcher { req: Some(req), global_state: self };
            readiness_dispatcher
                .on_sync_mut::<DocumentDiagnosticRequest>(
                    handle_document_diagnostic_before_workspace_ready,
                )
                .on_sync_mut::<WorkspaceDiagnosticRequest>(
                    handle_workspace_diagnostic_before_workspace_ready,
                )
                .on_sync_mut::<WorkspaceSymbolRequest>(
                    handle_workspace_symbol_before_workspace_ready,
                );

            let Some(pending_req) = readiness_dispatcher.req.take() else {
                return;
            };
            req = pending_req;
        }

        let mut dispatcher = ReqDispatcher { req: Some(req), global_state: self };

        // Handle shutdown req first
        dispatcher.on_sync_mut::<lsp_types::request::Shutdown>(|this, ()| {
            this.client.shutdown_requested = true;
            this.cancel_all_tasks();
            Ok(())
        });

        match &mut dispatcher {
            ReqDispatcher { req: Some(req), global_state: this }
                if this.client.shutdown_requested =>
            {
                this.respond(lsp_server::Response::new_err(
                    req.id.clone(),
                    lsp_server::ErrorCode::InvalidRequest as i32,
                    this.config_state
                        .config
                        .i18n
                        .text(keys::SERVER_SHUTDOWN_ALREADY_REQUESTED)
                        .to_owned(),
                ));
                return;
            }
            _ => (),
        }

        use handlers::request::*;
        use lsp_types::request::*;
        dispatcher
            .on_no_retry::<Completion>(handle_completion)
            .on_latency_sensitive::<SemanticTokensFullRequest>(handle_semantic_tokens_full)
            .on_latency_sensitive::<SemanticTokensFullDeltaRequest>(
                handle_semantic_tokens_full_delta,
            )
            .on_latency_sensitive::<SemanticTokensRangeRequest>(handle_semantic_tokens_range)
            .on::<DocumentSymbolRequest>(handle_document_symbol)
            .on::<WorkspaceSymbolRequest>(handle_workspace_symbol)
            .on::<FoldingRangeRequest>(handle_folding_ranges)
            .on::<DocumentDiagnosticRequest>(handle_document_diagnostic)
            .on::<WorkspaceDiagnosticRequest>(handle_workspace_diagnostic)
            .on_no_retry::<SignatureHelpRequest>(handle_signature_help)
            .on_no_retry::<InlayHintRequest>(handle_inlay_hint)
            .on_no_retry::<CodeLensRequest>(handle_code_lens)
            .on_no_retry::<CodeLensResolve>(handle_code_lens_resolve)
            .on_no_retry::<HoverRequest>(handle_hover)
            .on_no_retry::<GotoDefinition>(handle_goto_definition)
            .on_no_retry::<GotoDeclaration>(handle_goto_declaration)
            .on_no_retry::<GotoTypeDefinition>(handle_goto_type_definition)
            .on_no_retry::<CallHierarchyPrepare>(handle_prepare_call_hierarchy)
            .on_no_retry::<CallHierarchyIncomingCalls>(handle_call_hierarchy_incoming)
            .on_no_retry::<CallHierarchyOutgoingCalls>(handle_call_hierarchy_outgoing)
            .on_no_retry::<DocumentHighlightRequest>(handle_document_highlight)
            .on_no_retry::<References>(handle_references)
            .on_no_retry::<PrepareRenameRequest>(handle_prepare_rename)
            .on_no_retry::<Rename>(handle_rename)
            .on_fmt_thread::<Formatting>(handle_formatting)
            .on_fmt_thread::<RangeFormatting>(handle_range_formatting)
            .on_fmt_thread::<OnTypeFormatting>(handle_on_type_formatting)
            .on_no_retry::<CodeActionRequest>(handle_code_action)
            .on_no_retry::<CodeActionResolveRequest>(handle_code_action_resolve)
            .on_sync_mut::<ExecuteCommand>(handle_execute_command)
            .on::<SelectionRangeRequest>(handle_selection_range)
            .finish();
    }
}

fn handle_document_diagnostic_before_workspace_ready(
    state: &mut GlobalState,
    _: lsp_types::DocumentDiagnosticParams,
) -> anyhow::Result<DocumentDiagnosticReportResult> {
    state.workspace.workspace_vfs.defer_diagnostics_until_ready();
    Ok(DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
        related_documents: None,
        full_document_diagnostic_report: FullDocumentDiagnosticReport {
            result_id: None,
            items: Vec::new(),
        },
    })
    .into())
}

fn handle_workspace_diagnostic_before_workspace_ready(
    state: &mut GlobalState,
    _: lsp_types::WorkspaceDiagnosticParams,
) -> anyhow::Result<WorkspaceDiagnosticReportResult> {
    state.workspace.workspace_vfs.defer_diagnostics_until_ready();
    Ok(WorkspaceDiagnosticReportResult::Report(WorkspaceDiagnosticReport { items: Vec::new() }))
}

fn handle_workspace_symbol_before_workspace_ready(
    _: &mut GlobalState,
    _: lsp_types::WorkspaceSymbolParams,
) -> anyhow::Result<Option<WorkspaceSymbolResponse>> {
    Ok(Some(WorkspaceSymbolResponse::Flat(Vec::new())))
}
