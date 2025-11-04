use hir::{
    completion::{CompletionEntryKind, CompletionScope},
    semantics::Semantics,
};
use ide_db::root_db::RootDb;

use crate::completion::{
    CompletionConfig, CompletionContext, CompletionItem, CompletionItemKind, compute_score,
    render::render_scope_entry, result::dedup_by_label,
};

const BUILTIN_TYPES: &[(&str, &str)] = &[
    // Integer types
    ("bit", "2-state single-bit type"),
    ("byte", "2-state 8-bit signed integer"),
    ("shortint", "2-state 16-bit signed integer"),
    ("int", "2-state 32-bit signed integer"),
    ("longint", "2-state 64-bit signed integer"),
    ("integer", "4-state 32-bit signed integer"),
    ("time", "4-state 64-bit unsigned integer for time values"),
    // Logic types
    ("logic", "4-state single-bit type"),
    ("reg", "4-state variable type (deprecated, use logic)"),
    // Real types
    ("shortreal", "32-bit floating point"),
    ("real", "64-bit floating point"),
    ("realtime", "64-bit floating point for time values"),
    // String and event
    ("string", "Dynamic string type"),
    ("event", "Event synchronization type"),
    ("chandle", "Opaque handle to external data"),
    // Void
    ("void", "Void type (for functions)"),
];

/// Collect type reference completions for the current position
pub(crate) fn complete_type_reference(
    db: &RootDb,
    ctx: &CompletionContext,
    config: &CompletionConfig,
) -> Vec<CompletionItem> {
    let prefix = ctx.prefix();
    let mut items = Vec::new();

    // keywords first?
    items.extend(crate::completion::keywords::keyword_completions(prefix, Some(ctx), config));

    for (name, detail) in BUILTIN_TYPES {
        if name.starts_with(prefix) {
            items.push(CompletionItem {
                score: compute_score(
                    prefix,
                    name,
                    CompletionItemKind::Type,
                    Some(ctx),
                    Some(CompletionScope::Unit),
                ),
                label: name.to_string(),
                label_detail: None,
                detail: Some(detail.to_string()),
                insert_text: None,
                filter_text: None,
                kind: CompletionItemKind::Type,
                primary_edit: None,
                additional_edits: Vec::new(),
            });
        }
    }

    let sema = Semantics::new(db);
    let scope_entries = sema.scope_completions(ctx.position.file_id, ctx.position.offset);

    // In type reference context, we primarily want types, but if we're in an
    // expression (like the initialization part of a variable declaration), we
    // should show all entries. Check if we're in an expression context as well.
    let is_also_expression = ctx.is_in_expression_context();

    for scoped in scope_entries {
        if scoped.entry.kind == CompletionEntryKind::Type || is_also_expression {
            items.push(render_scope_entry(scoped.entry, Some(scoped.scope), prefix, Some(ctx)));
        }
    }

    dedup_by_label(items)
}
