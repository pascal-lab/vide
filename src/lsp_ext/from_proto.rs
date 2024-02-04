use line_index::{LineCol, TextRange, TextSize, WideLineCol};
use span::FilePosition;
use utils::{
    lines::{LineInfo, PositionEncoding},
    paths::AbsPathBuf,
};
use vfs::vfs_path::VfsPath;

use crate::global_state::snapshot::GlobalStateSnapshot;

pub(crate) fn vfs_path(url: &lsp_types::Url) -> anyhow::Result<vfs::vfs_path::VfsPath> {
    let path = url.to_file_path().map_err(|()| anyhow::format_err!("url is not a file"))?;
    Ok(VfsPath::from(AbsPathBuf::try_from(path).unwrap()))
}

pub(crate) fn abs_path(url: &lsp_types::Url) -> anyhow::Result<AbsPathBuf> {
    let path = url.to_file_path().map_err(|()| anyhow::format_err!("url is not a file"))?;
    Ok(AbsPathBuf::try_from(path).unwrap())
}

// convert position (line, col) to Offset
pub(crate) fn offset(
    line_index: &LineInfo,
    position: lsp_types::Position,
) -> anyhow::Result<TextSize> {
    let line_col = match line_index.encoding {
        PositionEncoding::Utf8 => LineCol { line: position.line, col: position.character },
        PositionEncoding::Wide(enc) => {
            let line_col = WideLineCol { line: position.line, col: position.character };
            line_index
                .index
                .to_utf8(enc, line_col)
                .ok_or_else(|| anyhow::format_err!("Invalid wide col offset"))?
        }
    };
    let text_size =
        line_index.index.offset(line_col).ok_or_else(|| anyhow::format_err!("Invalid offset"))?;
    Ok(text_size)
}

pub(crate) fn text_range(
    line_index: &LineInfo,
    range: lsp_types::Range,
) -> anyhow::Result<TextRange> {
    let start = offset(line_index, range.start)?;
    let end = offset(line_index, range.end)?;

    if end < start {
        return Err(anyhow::format_err!("Invalid Range").into());
    }

    Ok(TextRange::new(start, end))
}

pub(crate) fn file_position(
    snap: &GlobalStateSnapshot,
    pos_params: lsp_types::TextDocumentPositionParams,
) -> anyhow::Result<FilePosition> {
    let file_id = snap.file_id(&pos_params.text_document.uri)?;
    let line_index = snap.line_index(file_id)?;
    let offset = offset(&line_index, pos_params.position)?;
    Ok(FilePosition { file_id, offset })
}
