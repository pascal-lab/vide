use crate::{
    global_state::snapshot::GlobalStateSnapshot,
    lsp_ext::{from_proto, to_proto},
};

pub(crate) fn handle_completion(
    snap: GlobalStateSnapshot,
    params: lsp_types::CompletionParams,
) -> anyhow::Result<Option<lsp_types::CompletionResponse>> {
    use ide::completion::{CompletionItemKind as IdeCompletionItemKind, context::TriggerChar};
    use lsp_types::CompletionTextEdit;

    let position = from_proto::file_position(&snap, params.text_document_position)?;
    let line_info = snap.line_info(position.file_id)?;

    let trigger = params
        .context
        .as_ref()
        .and_then(|ctx| ctx.trigger_character.as_deref())
        .and_then(|s| s.chars().next())
        .and_then(|ch| match ch {
            '.' => Some(TriggerChar::Dot),
            '(' => Some(TriggerChar::OpenParen),
            ',' => Some(TriggerChar::Comma),
            '@' => Some(TriggerChar::At),
            '#' => Some(TriggerChar::Hash),
            '$' => Some(TriggerChar::Dollar),
            '`' => Some(TriggerChar::Backtick),
            '\'' => Some(TriggerChar::Apostrophe),
            '\n' | '\r' => Some(TriggerChar::Newline),
            _ => None,
        });

    let snippet_support = snap.config.cli_completion_snippet_support();
    let items = snap.analysis.completions_with_trigger(position, trigger)?;
    let items = items
        .into_iter()
        .filter_map(|item| {
            let sort_text = item.sort_text();
            let (edit, insert_text_format) = if snippet_support {
                match (item.snippet_edit, item.edit) {
                    (Some(edit), _) => Some((edit, Some(lsp_types::InsertTextFormat::SNIPPET))),
                    (None, Some(edit)) => Some((edit, None)),
                    (None, None) => None,
                }
            } else {
                item.edit.map(|edit| (edit, None))
            }?;

            let kind = match item.kind {
                IdeCompletionItemKind::Text => lsp_types::CompletionItemKind::TEXT,
                IdeCompletionItemKind::Keyword => lsp_types::CompletionItemKind::KEYWORD,
                IdeCompletionItemKind::Snippet => lsp_types::CompletionItemKind::SNIPPET,
            };

            Some(lsp_types::CompletionItem {
                label: item.label,
                kind: Some(kind),
                sort_text: Some(sort_text),
                insert_text_format,
                text_edit: Some(CompletionTextEdit::Edit(to_proto::text_edit(&line_info, edit))),
                ..Default::default()
            })
        })
        .collect();

    Ok(Some(lsp_types::CompletionResponse::Array(items)))
}
