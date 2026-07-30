use ide::Cancelled;
use lsp_server::{ErrorCode, Request, Response, ResponseError};
use lsp_types::{
    DocumentDiagnosticReport, DocumentDiagnosticReportResult, FullDocumentDiagnosticReport,
    RelatedFullDocumentDiagnosticReport, WorkspaceDiagnosticReport,
    WorkspaceDiagnosticReportResult, WorkspaceSymbolResponse,
    request::{
        DocumentDiagnosticRequest, Request as _, WorkspaceDiagnosticRequest, WorkspaceSymbolRequest,
    },
};
use utils::cancellation::{CancellationError, CancellationToken};
use vide_lsp_runtime::{AcceptedEffects, HandlerFailure, RequestPolicy, Router, RuntimeState};

use super::{
    GlobalState,
    handlers::{notification, request},
    response_effect::AcceptedResponseEffect,
    snapshot::GlobalStateSnapshot,
    task::Task,
};
use crate::lsp_ext::lsp_error::LspError;

pub(crate) type LspRouter = Router<GlobalState>;

pub(crate) fn router() -> LspRouter {
    use lsp_types::{notification::*, request::*};

    macro_rules! requests {
        ($router:ident; $($request:ty => $policy:expr, $handler:path;)*) => {
            $($router.request::<$request>($policy, $handler);)*
        };
    }
    macro_rules! notifications {
        ($router:ident; $($notification:ty => $handler:path;)*) => {
            $($router.notification::<$notification>($handler);)*
        };
    }

    let mut router = Router::new();
    router.request_mut::<Shutdown>(handle_shutdown);
    router.request_mut::<ExecuteCommand>(request::handle_execute_command);
    requests!(router;
        Completion => RequestPolicy::WORKER_NO_RETRY, request::handle_completion;
        SemanticTokensFullRequest => RequestPolicy::LATENCY_SENSITIVE, request::handle_semantic_tokens_full;
        SemanticTokensFullDeltaRequest => RequestPolicy::LATENCY_SENSITIVE, request::handle_semantic_tokens_full_delta;
        SemanticTokensRangeRequest => RequestPolicy::LATENCY_SENSITIVE, request::handle_semantic_tokens_range;
        DocumentSymbolRequest => RequestPolicy::WORKER, request::handle_document_symbol;
        WorkspaceSymbolRequest => RequestPolicy::WORKER, request::handle_workspace_symbol;
        FoldingRangeRequest => RequestPolicy::WORKER, request::handle_folding_ranges;
        DocumentDiagnosticRequest => RequestPolicy::WORKER, request::handle_document_diagnostic;
        WorkspaceDiagnosticRequest => RequestPolicy::WORKER, request::handle_workspace_diagnostic;
        SignatureHelpRequest => RequestPolicy::WORKER_NO_RETRY, request::handle_signature_help;
        InlayHintRequest => RequestPolicy::WORKER_NO_RETRY, request::handle_inlay_hint;
        CodeLensRequest => RequestPolicy::WORKER_NO_RETRY, request::handle_code_lens;
        CodeLensResolve => RequestPolicy::WORKER_NO_RETRY, request::handle_code_lens_resolve;
        HoverRequest => RequestPolicy::WORKER_NO_RETRY, request::handle_hover;
        GotoDefinition => RequestPolicy::WORKER_NO_RETRY, request::handle_goto_definition;
        GotoDeclaration => RequestPolicy::WORKER_NO_RETRY, request::handle_goto_declaration;
        GotoTypeDefinition => RequestPolicy::WORKER_NO_RETRY, request::handle_goto_type_definition;
        CallHierarchyPrepare => RequestPolicy::WORKER_NO_RETRY, request::handle_prepare_call_hierarchy;
        CallHierarchyIncomingCalls => RequestPolicy::WORKER_NO_RETRY, request::handle_call_hierarchy_incoming;
        CallHierarchyOutgoingCalls => RequestPolicy::WORKER_NO_RETRY, request::handle_call_hierarchy_outgoing;
        DocumentHighlightRequest => RequestPolicy::WORKER_NO_RETRY, request::handle_document_highlight;
        References => RequestPolicy::WORKER_NO_RETRY, request::handle_references;
        PrepareRenameRequest => RequestPolicy::WORKER_NO_RETRY, request::handle_prepare_rename;
        Rename => RequestPolicy::WORKER_NO_RETRY, request::handle_rename;
        Formatting => RequestPolicy::LATENCY_SENSITIVE_NO_RETRY, request::handle_formatting;
        RangeFormatting => RequestPolicy::LATENCY_SENSITIVE_NO_RETRY, request::handle_range_formatting;
        OnTypeFormatting => RequestPolicy::LATENCY_SENSITIVE_NO_RETRY, request::handle_on_type_formatting;
        CodeActionRequest => RequestPolicy::WORKER_NO_RETRY, request::handle_code_action;
        CodeActionResolveRequest => RequestPolicy::WORKER_NO_RETRY, request::handle_code_action_resolve;
        SelectionRangeRequest => RequestPolicy::WORKER, request::handle_selection_range;
    );
    notifications!(router;
        Cancel => notification::handle_cancel;
        WorkDoneProgressCancel => notification::handle_work_done_progress_cancel;
        DidOpenTextDocument => notification::handle_did_open_text_document;
        DidChangeTextDocument => notification::handle_did_change_text_document;
        DidCloseTextDocument => notification::handle_did_close_text_document;
        DidSaveTextDocument => notification::handle_did_save_text_document;
        DidChangeConfiguration => notification::handle_did_change_configuration;
        DidChangeWorkspaceFolders => notification::handle_did_change_workspace_folders;
        DidChangeWatchedFiles => notification::handle_did_change_watched_files;
        SetTrace => notification::handle_set_trace;
    );
    router
}

impl GlobalState {
    pub(crate) fn dispatch_request(&mut self, router: &LspRouter, request: Request) {
        if !self.is_workspace_ready() && self.respond_before_workspace_ready(&request) {
            return;
        }
        router.dispatch_request(self, request);
    }

    pub(crate) fn dispatch_notification(
        &mut self,
        router: &LspRouter,
        notification: lsp_server::Notification,
    ) {
        router.dispatch_notification(self, notification);
    }

    fn respond_before_workspace_ready(&mut self, request: &Request) -> bool {
        let result = match request.method.as_str() {
            DocumentDiagnosticRequest::METHOD => {
                self.workspace.workspace_vfs.defer_diagnostics_until_ready();
                let result: DocumentDiagnosticReportResult =
                    DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
                        related_documents: None,
                        full_document_diagnostic_report: FullDocumentDiagnosticReport {
                            result_id: None,
                            items: Vec::new(),
                        },
                    })
                    .into();
                serde_json::to_value(result)
            }
            WorkspaceDiagnosticRequest::METHOD => {
                self.workspace.workspace_vfs.defer_diagnostics_until_ready();
                serde_json::to_value(WorkspaceDiagnosticReportResult::Report(
                    WorkspaceDiagnosticReport { items: Vec::new() },
                ))
            }
            WorkspaceSymbolRequest::METHOD => {
                serde_json::to_value(Some(WorkspaceSymbolResponse::Flat(Vec::new())))
            }
            _ => return false,
        };

        let response = match result {
            Ok(result) => Response { id: request.id.clone(), result: Some(result), error: None },
            Err(error) => Response::new_err(
                request.id.clone(),
                ErrorCode::InternalError as i32,
                error.to_string(),
            ),
        };
        self.client.respond(response);
        true
    }
}

impl RuntimeState for GlobalState {
    type Effect = AcceptedResponseEffect;
    type Snapshot = GlobalStateSnapshot;

    fn client(&self) -> &vide_lsp_runtime::Client<Self> {
        &self.client
    }

    fn snapshot(&self, cancellation: CancellationToken) -> Self::Snapshot {
        self.make_snapshot_with_cancel(cancellation)
    }

    fn accepted_effects(snapshot: &Self::Snapshot) -> AcceptedEffects<Self::Effect> {
        snapshot.accepted_response_effects()
    }

    fn spawn_protocol_task(
        &mut self,
        intent: utils::thread::ThreadIntent,
        task: Box<dyn FnOnce() -> vide_lsp_runtime::ProtocolTask<Self::Effect> + Send>,
    ) {
        self.tasks.task_pool.handle.spawn_and_send(intent, move || Task::Protocol(task()));
    }

    fn map_handler_error(
        error: anyhow::Error,
        _cancellation: &CancellationToken,
    ) -> HandlerFailure {
        match error.downcast::<CancellationError>() {
            Ok(_) => HandlerFailure::Cancelled,
            Err(error) => match error.downcast::<LspError>() {
                Ok(error) => HandlerFailure::Response(ResponseError {
                    code: error.code,
                    message: error.message,
                    data: None,
                }),
                Err(error) => match error.downcast::<Cancelled>() {
                    Ok(_) => HandlerFailure::Cancelled,
                    Err(error) => HandlerFailure::Response(ResponseError {
                        code: ErrorCode::InternalError as i32,
                        message: error.to_string(),
                        data: None,
                    }),
                },
            },
        }
    }
}

fn handle_shutdown(state: &mut GlobalState, (): ()) -> anyhow::Result<()> {
    state.client.request_shutdown();
    Ok(())
}
