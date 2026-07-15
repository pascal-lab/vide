use std::ops::Range;

use triomphe::Arc;
use utils::{
    line_index::LineIndex,
    lines::{LineEnding, LineInfo, PositionEncoding},
};
use vfs::{FileId, VfsPath};

use crate::{global_state::GlobalState, lsp_ext::from_proto};

pub(super) fn set_vfs_file_contents(
    state: &mut GlobalState,
    path: &VfsPath,
    text: String,
) -> anyhow::Result<FileId> {
    let (text, endings) = LineEnding::normalize(text);
    let mut vfs = state.workspace.vfs.write();
    let path = path.clone();
    vfs.0.set_file_contents(path.clone(), Some(text.into_bytes()));
    let file_id = vfs
        .0
        .file_id(&path)
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::format_err!("loaded file has no FileId: {path}"))?;
    vfs.1.insert(file_id, endings);
    Ok(file_id)
}

pub(super) fn open_vfs_file_contents(
    state: &mut GlobalState,
    path: &VfsPath,
    text: &str,
) -> anyhow::Result<FileId> {
    let mut vfs = state.workspace.vfs.write();
    if let Some((file_id, _)) = vfs.0.file_id(path)
        && state.analysis.mem_docs.contains_file_id(file_id)
    {
        return Ok(file_id);
    }

    let (text, endings) = LineEnding::normalize(text.to_owned());
    let path = path.clone();
    vfs.0.set_file_contents(path.clone(), Some(text.into_bytes()));
    let file_id = vfs
        .0
        .file_id(&path)
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::format_err!("loaded file has no FileId: {path}"))?;
    vfs.1.insert(file_id, endings);
    Ok(file_id)
}

pub(super) fn open_mem_doc_file_id(state: &GlobalState, path: &VfsPath) -> Option<FileId> {
    state.analysis.mem_docs.file_id(path).or_else(|| {
        state.workspace.vfs.read().0.file_id(path).and_then(|(file_id, _)| {
            state.analysis.mem_docs.contains_file_id(file_id).then_some(file_id)
        })
    })
}

pub(super) fn update_document_text(
    encoding: PositionEncoding,
    data: &str,
    content_changes: Vec<lsp_types::TextDocumentContentChangeEvent>,
) -> anyhow::Result<Option<String>> {
    let text = apply_document_changes(encoding, data, content_changes)?;

    if data == text { Ok(None) } else { Ok(Some(text)) }
}

fn apply_document_changes(
    encoding: PositionEncoding,
    file_contents: &str,
    content_changes: Vec<lsp_types::TextDocumentContentChangeEvent>,
) -> anyhow::Result<String> {
    // Skip to the last full document change and peek at the first content change.
    let (mut text, content_changes) = {
        match content_changes.iter().rposition(|change| change.range.is_none()) {
            Some(idx) => {
                let (full_doc_changes, rest) = content_changes.split_at(idx + 1);
                match full_doc_changes.last() {
                    Some(full_doc_change) => (full_doc_change.text.clone(), rest),
                    None => (file_contents.to_owned(), rest),
                }
            }
            None => (file_contents.to_owned(), &content_changes[..]),
        }
    };

    if content_changes.is_empty() {
        return Ok(text);
    }

    // The changes can cross lines so we have to keep our line index updated.
    // Here's an optimization: we only rebuild the index if we have to, iff
    // the change's start line is greater than the last valid line.
    // The VFS will normalize the end of lines to `\n`.
    let mut line_info = LineInfo {
        index: Arc::new(LineIndex::new(&text)),
        // We don't care about line endings here.
        ending: LineEnding::Unix,
        encoding,
    };

    // Set to infinity at first, to avoid rebuilding the index on the first change.
    let mut index_valid_until = !0u32;
    for change in content_changes {
        let Some(range) = change.range else {
            text = change.text.clone();
            *Arc::make_mut(&mut line_info.index) = LineIndex::new(&text);
            index_valid_until = !0u32;
            continue;
        };
        if index_valid_until <= range.end.line {
            *Arc::make_mut(&mut line_info.index) = LineIndex::new(&text);
        }
        index_valid_until = range.start.line;
        let range = from_proto::text_range(&line_info, range)?;
        text.replace_range(Range::<usize>::from(range), &change.text);
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use lsp_types::TextDocumentContentChangeEvent;
    use utils::lines::PositionEncoding;

    use super::update_document_text;

    #[test]
    fn clearing_document_updates_mem_doc_and_vfs_text() {
        let text = "module top;\nendmodule\n".to_owned();
        let vfs_text = update_document_text(
            PositionEncoding::Utf8,
            &text,
            vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: String::new(),
            }],
        )
        .unwrap();

        assert_eq!(vfs_text.as_deref(), Some(""));
    }

    #[test]
    fn unchanged_document_skips_vfs_update() {
        let text = "module top;\nendmodule\n".to_owned();
        let vfs_text = update_document_text(
            PositionEncoding::Utf8,
            &text,
            vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "module top;\nendmodule\n".to_owned(),
            }],
        )
        .unwrap();

        assert!(vfs_text.is_none());
    }

    #[test]
    fn invalid_range_change_does_not_apply_partial_text() {
        let text = "module top;\nendmodule\n".to_owned();
        let result = update_document_text(
            PositionEncoding::Utf8,
            &text,
            vec![TextDocumentContentChangeEvent {
                range: Some(lsp_types::Range::new(
                    lsp_types::Position::new(99, 0),
                    lsp_types::Position::new(99, 1),
                )),
                range_length: None,
                text: "broken".to_owned(),
            }],
        );

        assert!(result.is_err());
    }
}
