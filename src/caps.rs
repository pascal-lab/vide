use lsp_types::{
    ClientCapabilities, CodeActionKind, CodeActionOptions, CodeLensOptions, CompletionOptions,
    CompletionOptionsCompletionItem, DeclarationCapability, DocumentOnTypeFormattingOptions,
    FileOperationFilter, FileOperationPattern, FileOperationPatternKind,
    FileOperationRegistrationOptions, InlayHintOptions, InlayHintServerCapabilities, OneOf,
    PositionEncodingKind, RenameOptions, SaveOptions, SemanticTokensFullOptions,
    SemanticTokensLegend, SemanticTokensOptions, ServerCapabilities, SignatureHelpOptions,
    TextDocumentSyncKind, TextDocumentSyncOptions, WorkDoneProgressOptions,
    WorkspaceFileOperationsServerCapabilities, WorkspaceFoldersServerCapabilities,
    WorkspaceServerCapabilities,
};

use crate::{
    config::Config,
    line_idx::{PositionEncoding, WideEncoding},
};

macro_rules! try_ {
    ($expr:expr) => {
        || -> _ { Some($expr) }()
    };
}

macro_rules! try_or_default {
    ($expr:expr) => {
        try_!($expr).unwrap_or_default()
    };
}

impl Config {
    pub fn cli_completion_label_details_support(&self) -> bool {
        try_!(self
            .client_caps
            .text_document
            .as_ref()?
            .completion
            .as_ref()?
            .completion_item
            .as_ref()?
            .label_details_support
            .as_ref()?)
        .is_some()
    }

    pub fn cli_completion_item_edit_resolve(&self) -> bool {
        try_!(self
            .client_caps
            .text_document
            .as_ref()?
            .completion
            .as_ref()?
            .completion_item
            .as_ref()?
            .resolve_support
            .as_ref()?
            .properties
            .iter()
            .any(|cap_string| cap_string.as_str() == "additionalTextEdits"))
            == Some(true)
    }

    pub fn cli_did_save_dyn_reg_support(&self) -> bool {
        let caps =
            try_or_default!(self.client_caps.text_document.as_ref()?.synchronization.clone()?);
        caps.did_save == Some(true) && caps.dynamic_registration == Some(true)
    }

    pub fn get_server_capabilities(&self) -> ServerCapabilities {
        ServerCapabilities {
            position_encoding: match self.negotiated_encoding() {
                PositionEncoding::Utf8 => Some(PositionEncodingKind::UTF8),
                PositionEncoding::Wide(wide) => match wide {
                    WideEncoding::Utf16 => Some(PositionEncodingKind::UTF16),
                    WideEncoding::Utf32 => Some(PositionEncodingKind::UTF32),
                },
            },
            text_document_sync: Some(
                TextDocumentSyncOptions {
                    open_close: true.into(),
                    change: TextDocumentSyncKind::INCREMENTAL.into(),
                    will_save: None,
                    will_save_wait_until: None,
                    save: Some(SaveOptions::default().into()),
                }
                .into(),
            ),
            selection_range_provider: Some(true.into()),
            hover_provider: Some(true.into()),
            completion_provider: CompletionOptions {
                resolve_provider: self.cli_completion_item_edit_resolve().into(),
                trigger_characters: Some([":", ",", "'", "(", "`"].map(String::from).into()),
                all_commit_characters: None,
                completion_item: CompletionOptionsCompletionItem {
                    label_details_support: self.cli_completion_label_details_support().into(),
                }
                .into(),
                work_done_progress_options: WorkDoneProgressOptions { work_done_progress: None },
            }
            .into(),
            signature_help_provider: SignatureHelpOptions {
                trigger_characters: Some(["(", ",", "`"].map(String::from).into()),
                retrigger_characters: None,
                work_done_progress_options: WorkDoneProgressOptions { work_done_progress: None },
            }
            .into(),
            declaration_provider: Some(DeclarationCapability::Simple(true)),
            definition_provider: OneOf::Left(true).into(),
            type_definition_provider: Some(true.into()),
            implementation_provider: Some(true.into()),
            references_provider: OneOf::Left(true).into(),
            document_highlight_provider: OneOf::Left(true).into(),
            document_symbol_provider: OneOf::Left(true).into(),
            workspace_symbol_provider: OneOf::Left(true).into(),
            code_action_provider: Some({
                try_!(self
                    .client_caps
                    .text_document
                    .as_ref()?
                    .code_action
                    .as_ref()?
                    .code_action_literal_support
                    .as_ref()?)
                .map_or(true.into(), |_| {
                    CodeActionOptions {
                        code_action_kinds: vec![
                            CodeActionKind::EMPTY,
                            CodeActionKind::QUICKFIX,
                            CodeActionKind::REFACTOR,
                            CodeActionKind::REFACTOR_EXTRACT,
                            CodeActionKind::REFACTOR_INLINE,
                            CodeActionKind::REFACTOR_REWRITE,
                        ]
                        .into(),
                        resolve_provider: true.into(),
                        work_done_progress_options: Default::default(),
                    }
                    .into()
                })
            }),
            code_lens_provider: CodeLensOptions { resolve_provider: true.into() }.into(),
            document_formatting_provider: OneOf::Left(true).into(),
            document_range_formatting_provider: OneOf::Left(true).into(),
            document_on_type_formatting_provider: DocumentOnTypeFormattingOptions {
                first_trigger_character: "=".to_string(),
                more_trigger_character: Some([".", ">", "{", "(", "<"].map(String::from).into()),
            }
            .into(),
            rename_provider: OneOf::Right(RenameOptions {
                prepare_provider: true.into(),
                work_done_progress_options: WorkDoneProgressOptions { work_done_progress: None },
            })
            .into(),
            document_link_provider: None,
            color_provider: None,
            folding_range_provider: Some(true.into()),
            execute_command_provider: None,
            workspace: WorkspaceServerCapabilities {
                workspace_folders: WorkspaceFoldersServerCapabilities {
                    supported: true.into(),
                    change_notifications: OneOf::Left(true).into(),
                }
                .into(),
                file_operations: WorkspaceFileOperationsServerCapabilities {
                    did_create: None,
                    will_create: None,
                    did_rename: None,
                    will_rename: FileOperationRegistrationOptions {
                        filters: vec![
                            FileOperationFilter {
                                scheme: String::from("file").into(),
                                pattern: FileOperationPattern {
                                    glob: String::from("**/*.rs"),
                                    matches: FileOperationPatternKind::File.into(),
                                    options: None,
                                },
                            },
                            FileOperationFilter {
                                scheme: String::from("file").into(),
                                pattern: FileOperationPattern {
                                    glob: String::from("**"),
                                    matches: FileOperationPatternKind::Folder.into(),
                                    options: None,
                                },
                            },
                        ],
                    }
                    .into(),
                    did_delete: None,
                    will_delete: None,
                }
                .into(),
            }
            .into(),
            call_hierarchy_provider: Some(true.into()),
            semantic_tokens_provider: Some(
                SemanticTokensOptions {
                    // TODO:
                    legend: SemanticTokensLegend::default(),
                    // {
                    //     token_types: SemanticTokensLegend::
                    //     token_modifiers: semantic_tokens::SUPPORTED_MODIFIERS.to_vec(),
                    // },
                    full: SemanticTokensFullOptions::Delta { delta: true.into() }.into(),
                    range: true.into(),
                    work_done_progress_options: Default::default(),
                }
                .into(),
            ),
            moniker_provider: None,
            linked_editing_range_provider: None,
            inline_value_provider: None,
            inlay_hint_provider: OneOf::Right(InlayHintServerCapabilities::Options(
                InlayHintOptions {
                    work_done_progress_options: Default::default(),
                    resolve_provider: true.into(),
                },
            ))
            .into(),
            diagnostic_provider: None,
            experimental: None,
        }
    }

    fn negotiated_encoding(&self) -> PositionEncoding {
        let client_encodings = match &self.client_caps.general {
            Some(general) => general.position_encodings.as_deref().unwrap_or_default(),
            None => &[],
        };

        for enc in client_encodings {
            if enc == &PositionEncodingKind::UTF8 {
                return PositionEncoding::Utf8;
            } else if enc == &PositionEncodingKind::UTF32 {
                return PositionEncoding::Wide(WideEncoding::Utf32);
            }
            // NB: intentionally prefer just about anything else to utf-16.
        }

        PositionEncoding::Wide(WideEncoding::Utf16)
    }
}
