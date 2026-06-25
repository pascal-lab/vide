mod code_action;
mod commands;
mod completion;
mod diagnostics;
mod formatting;
mod hints_lens;
mod navigation;
mod rename;
mod semantic_tokens;
mod symbols;

pub(crate) use code_action::{handle_code_action, handle_code_action_resolve};
pub(crate) use commands::handle_execute_command;
pub(crate) use completion::handle_completion;
pub(crate) use diagnostics::{handle_document_diagnostic, handle_workspace_diagnostic};
pub(crate) use formatting::{
    handle_formatting, handle_on_type_formatting, handle_range_formatting,
};
pub(crate) use hints_lens::{
    handle_code_lens, handle_code_lens_resolve, handle_inlay_hint, handle_signature_help,
};
pub(crate) use navigation::{
    handle_call_hierarchy_incoming, handle_call_hierarchy_outgoing, handle_document_highlight,
    handle_goto_declaration, handle_goto_definition, handle_goto_type_definition, handle_hover,
    handle_prepare_call_hierarchy, handle_references,
};
pub(crate) use rename::{handle_prepare_rename, handle_rename};
pub(crate) use semantic_tokens::{
    handle_semantic_tokens_full, handle_semantic_tokens_full_delta, handle_semantic_tokens_range,
};
pub(crate) use symbols::{
    handle_document_symbol, handle_folding_ranges, handle_selection_range, handle_workspace_symbol,
};
