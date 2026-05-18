use ide::hover::HoverFormat;
use lsp_types::{
    CodeActionKind, CodeActionOptions, CodeActionProviderCapability, CodeLensOptions,
    DeclarationCapability, DiagnosticOptions, DiagnosticServerCapabilities,
    DocumentOnTypeFormattingOptions, FileOperationFilter, FileOperationPattern,
    FileOperationPatternKind, FileOperationRegistrationOptions, InlayHintOptions,
    InlayHintServerCapabilities, OneOf, PositionEncodingKind, RenameOptions, SaveOptions,
    SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions, ServerCapabilities,
    SignatureHelpOptions, TextDocumentSyncKind, TextDocumentSyncOptions,
    WorkspaceFileOperationsServerCapabilities, WorkspaceFoldersServerCapabilities,
    WorkspaceServerCapabilities,
};
use utils::{line_index::WideEncoding, lines::PositionEncoding, try_, try_or_default};

use crate::{
    config::Config,
    lsp_ext::ext::{self, RUN_QIHE_ANALYSIS_COMMAND},
};

impl Config {
    pub fn cli_completion_label_details_support(&self) -> bool {
        try_!(
            self.client_caps
                .text_document
                .as_ref()?
                .completion
                .as_ref()?
                .completion_item
                .as_ref()?
                .label_details_support
                .as_ref()?
        )
        .is_some()
    }

    pub fn cli_completion_item_edit_resolve(&self) -> bool {
        try_!(
            self.client_caps
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
                .any(|cap_string| cap_string.as_str() == "additionalTextEdits")
        ) == Some(true)
    }

    pub fn cli_completion_snippet_support(&self) -> bool {
        try_or_default!(
            self.client_caps
                .text_document
                .as_ref()?
                .completion
                .as_ref()?
                .completion_item
                .as_ref()?
                .snippet_support?
        )
    }

    pub fn hierarchical_symbols(&self) -> bool {
        try_!(
            self.client_caps
                .text_document
                .as_ref()?
                .document_symbol
                .as_ref()?
                .hierarchical_document_symbol_support?
        )
        .unwrap_or_default()
    }

    pub fn location_link(&self) -> bool {
        try_or_default!(self.client_caps.text_document.as_ref()?.definition?.link_support?)
    }

    pub fn cli_did_save_dyn_reg(&self) -> bool {
        let caps =
            try_or_default!(self.client_caps.text_document.as_ref()?.synchronization.clone()?);
        caps.did_save == Some(true) && caps.dynamic_registration == Some(true)
    }

    pub fn cli_did_change_watched_files_dyn_reg(&self) -> bool {
        try_or_default!(
            self.client_caps
                .workspace
                .as_ref()?
                .did_change_watched_files
                .as_ref()?
                .dynamic_registration?
        )
    }

    pub fn cli_work_done_progress(&self) -> bool {
        try_or_default!(self.client_caps.window.as_ref()?.work_done_progress?)
    }

    pub fn cli_line_folding_only(&self) -> bool {
        try_or_default! {
            self.client_caps
            .text_document.as_ref()?
            .folding_range.as_ref()?
            .line_folding_only?
        }
    }

    pub fn cli_hover_markdown_support(&self) -> HoverFormat {
        let support_markdown = try_or_default! {
            self.client_caps
            .text_document.as_ref()?
            .hover.as_ref()?
            .content_format.as_ref()?
            .contains(&lsp_types::MarkupKind::Markdown)
        };

        if support_markdown { HoverFormat::Markdown } else { HoverFormat::PlainText }
    }

    pub fn cli_inlay_hint_refresh_support(&self) -> bool {
        try_or_default! {
            self.client_caps
            .workspace.as_ref()?
            .inlay_hint.as_ref()?
            .refresh_support?
        }
    }

    pub fn cli_code_lens_refresh_support(&self) -> bool {
        try_or_default! {
            self.client_caps
            .workspace.as_ref()?
            .code_lens.as_ref()?
            .refresh_support?
        }
    }

    pub fn cli_workspace_diagnostic_refresh_support(&self) -> bool {
        try_or_default! {
            self.client_caps
            .workspace.as_ref()?
            .diagnostic.as_ref()?
            .refresh_support?
        }
    }

    pub fn cli_pull_diagnostics_support(&self) -> bool {
        try_!(self.client_caps.text_document.as_ref()?.diagnostic.as_ref()).is_some()
    }

    pub fn cli_signature_help_label_offsets_support(&self) -> bool {
        try_or_default! {
            self.client_caps
                .text_document
                .as_ref()?
                .signature_help
                .as_ref()?
                .signature_information
                .as_ref()?
                .parameter_information
                .as_ref()?
                .label_offset_support?
        }
    }

    pub fn cli_code_action_literals(&self) -> bool {
        try_or_default! {
            self.client_caps
            .text_document
            .as_ref()?
            .code_action
            .as_ref()?
            .code_action_literal_support
            .as_ref()
        }
        .is_some()
    }

    pub fn cli_code_action_resolve(&self) -> bool {
        try_or_default! {
            self.client_caps
            .text_document
            .as_ref()?
            .code_action
            .as_ref()?
            .resolve_support
            .as_ref()?
            .properties
            .iter()
            .any(|it| it.as_str() == "edit")
        }
    }

    pub(crate) fn negotiated_encoding(&self) -> PositionEncoding {
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

impl Config {
    pub fn server_caps(&self) -> ServerCapabilities {
        ServerCapabilities {
            position_encoding: match self.negotiated_encoding() {
                PositionEncoding::Utf8 => Some(PositionEncodingKind::UTF8),
                PositionEncoding::Wide(wide) => match wide {
                    WideEncoding::Utf16 => Some(PositionEncodingKind::UTF16),
                    WideEncoding::Utf32 => Some(PositionEncodingKind::UTF32),
                    _ => None,
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
            completion_provider: Some(
                lsp_types::CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![
                        ".".into(),
                        "(".into(),
                        ",".into(),
                        "@".into(),
                        "#".into(),
                        "`".into(),
                    ]),
                    ..Default::default()
                }
                .into(),
            ),
            signature_help_provider: SignatureHelpOptions {
                trigger_characters: Some(["(", ",", "."].map(String::from).into()),
                retrigger_characters: None,
                work_done_progress_options: Default::default(),
            }
            .into(),
            declaration_provider: Some(DeclarationCapability::Simple(true)),
            definition_provider: OneOf::Left(true).into(),
            type_definition_provider: Some(true.into()),
            implementation_provider: Some(false.into()),
            references_provider: OneOf::Left(true).into(),
            document_highlight_provider: OneOf::Left(true).into(),
            document_symbol_provider: OneOf::Left(true).into(),
            workspace_symbol_provider: OneOf::Left(true).into(),
            code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
                code_action_kinds: Some(vec![
                    CodeActionKind::EMPTY,
                    CodeActionKind::QUICKFIX,
                    CodeActionKind::REFACTOR,
                    CodeActionKind::REFACTOR_EXTRACT,
                    CodeActionKind::REFACTOR_INLINE,
                    CodeActionKind::REFACTOR_REWRITE,
                ]),
                work_done_progress_options: Default::default(),
                resolve_provider: Some(true),
            })),
            code_lens_provider: CodeLensOptions { resolve_provider: true.into() }.into(),
            document_formatting_provider: OneOf::Left(true).into(),
            document_range_formatting_provider: OneOf::Left(true).into(),
            document_on_type_formatting_provider: DocumentOnTypeFormattingOptions {
                first_trigger_character: "\n".to_owned(),
                more_trigger_character: None,
            }
            .into(),
            rename_provider: OneOf::Right(RenameOptions {
                prepare_provider: true.into(),
                work_done_progress_options: Default::default(),
            })
            .into(),
            document_link_provider: None,
            color_provider: None,
            folding_range_provider: Some(true.into()),
            execute_command_provider: Some(lsp_types::ExecuteCommandOptions {
                commands: vec![RUN_QIHE_ANALYSIS_COMMAND.to_string()],
                work_done_progress_options: Default::default(),
            }),
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
                                    glob: String::from("**/*.{v,sv}"),
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
                    legend: SemanticTokensLegend {
                        token_types: ext::SEMA_TOKENS_TYPES.to_vec(),
                        token_modifiers: ext::SEMA_TOKENS_MODIFIERS.to_vec(),
                    },

                    full: Some(SemanticTokensFullOptions::Delta { delta: Some(true) }),
                    range: Some(true),
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
                    resolve_provider: false.into(),
                },
            ))
            .into(),
            diagnostic_provider: Some(DiagnosticServerCapabilities::Options(DiagnosticOptions {
                identifier: None,
                inter_file_dependencies: true,
                workspace_diagnostics: true,
                work_done_progress_options: Default::default(),
            })),
            experimental: None,
        }
    }
}
