mod action;
mod collector;
mod context;
mod diagnostics;
mod edits;
mod engine;
mod handlers;
mod module_members;

pub use action::{CodeAction, CodeActionId, CodeActionKind, CodeActionResolveStrategy};
pub(crate) use collector::CodeActionCollector;
pub(crate) use context::CodeActionCtx;
pub use diagnostics::{
    CodeActionDiagnostic, CodeActionDiagnostics, DiagnosticCode, DiagnosticSource, RepairKind,
};
pub(crate) use edits::{apply_missing_list_edit, line_indent, newline_style, text_at};
pub(crate) use engine::code_action;
pub(crate) use module_members::{
    all_parameter_names, leading_parameter_names, missing_member_entry_text, port_names,
    remaining_ordered_port_names,
};

#[cfg(test)]
mod tests;
