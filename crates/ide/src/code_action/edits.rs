use memchr::memrchr;
use utils::text_edit::{TextRange, TextSize};

use crate::source_change::SourceChangeBuilder;

struct MissingListEdit {
    range: TextRange,
    replacement: String,
}

pub(crate) fn apply_missing_list_edit(
    builder: &mut SourceChangeBuilder,
    text: &str,
    open_paren: TextRange,
    close_paren: TextRange,
    item_ranges: impl IntoIterator<Item = TextRange>,
    entries: Vec<String>,
) {
    if let Some(edit) = missing_list_edit(text, open_paren, close_paren, item_ranges, entries) {
        builder.replace(edit.range, edit.replacement);
    }
}

fn missing_list_edit(
    text: &str,
    open_paren: TextRange,
    close_paren: TextRange,
    item_ranges: impl IntoIterator<Item = TextRange>,
    entries: Vec<String>,
) -> Option<MissingListEdit> {
    if entries.is_empty() {
        return None;
    }

    let open_end = open_paren.end();
    let close_start = close_paren.start();
    if close_start < open_end {
        return None;
    }

    let open_end_usize = usize::from(open_end);
    let close_start_usize = usize::from(close_start);
    let content = text.get(open_end_usize..close_start_usize)?;
    let multiline = content.contains('\n');

    let trimmed_len = content.trim_end_matches(char::is_whitespace).len();
    let trimmed = &content[..trimmed_len];
    let trailing_comma = trimmed.ends_with(',');
    let meaningful_len =
        if trailing_comma { trimmed.len().saturating_sub(1) } else { trimmed.len() };
    let has_existing_text = !trimmed[..meaningful_len].trim().is_empty();

    let last_token_end = TextSize::from((open_end_usize + trimmed.len()) as u32);
    let range_start = if trailing_comma {
        last_token_end
    } else if has_existing_text {
        TextSize::from((open_end_usize + meaningful_len) as u32)
    } else {
        open_end
    };
    let range = TextRange::new(range_start, close_start);

    let replacement = if multiline {
        let close_indent = line_indent(text, close_start);
        let item_indent = item_ranges
            .into_iter()
            .filter(|range| !range.is_empty() && range.start() < close_start)
            .last()
            .and_then(|range| item_line_indent(text, range.start()))
            .unwrap_or_else(|| format!("{close_indent}    "));

        let mut lines = Vec::new();
        let entries_len = entries.len();
        for (idx, entry) in entries.into_iter().enumerate() {
            let needs_comma = trailing_comma || idx + 1 < entries_len;
            let comma = if needs_comma { "," } else { "" };
            lines.push(format!("{item_indent}{entry}{comma}"));
        }

        let rendered_entries = lines.join("\n");
        let prefix = if has_existing_text && !trailing_comma { "," } else { "" };
        format!("{prefix}\n{rendered_entries}\n{close_indent}")
    } else {
        let rendered_entries = entries.join(", ");
        if has_existing_text {
            let separator = if trailing_comma { " " } else { ", " };
            format!("{separator}{rendered_entries}")
        } else {
            rendered_entries
        }
    };

    Some(MissingListEdit { range, replacement })
}

pub(crate) fn line_indent(text: &str, offset: TextSize) -> String {
    let offset = usize::from(offset).min(text.len());
    let line_start = line_start(text, offset);
    let indent_end = leading_indent_end(text, line_start, offset);
    text[line_start..indent_end].to_owned()
}

fn item_line_indent(text: &str, offset: TextSize) -> Option<String> {
    let offset = usize::from(offset).min(text.len());
    let line_start = line_start(text, offset);
    (leading_indent_end(text, line_start, offset) == offset)
        .then(|| text[line_start..offset].to_owned())
}

fn line_start(text: &str, offset: usize) -> usize {
    memrchr(b'\n', &text.as_bytes()[..offset]).map(|idx| idx + 1).unwrap_or(0)
}

fn leading_indent_end(text: &str, start: usize, end: usize) -> usize {
    let bytes = text.as_bytes();
    let mut idx = start;
    while idx < end && matches!(bytes[idx], b' ' | b'\t') {
        idx += 1;
    }
    idx
}
