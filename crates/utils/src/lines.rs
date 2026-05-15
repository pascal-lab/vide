use triomphe::Arc;

use crate::line_index::{LineIndex, WideEncoding};

#[derive(Clone, Copy)]
pub enum PositionEncoding {
    Utf8,
    Wide(WideEncoding),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LineEnding {
    Unix,
    Dos,
}

impl LineEnding {
    /// Replaces `\r\n` with `\n` in-place in `src`.
    pub fn normalize(src: String) -> (String, LineEnding) {
        if src.contains("\r\n") {
            (src.replace("\r\n", "\n"), LineEnding::Dos)
        } else {
            (src, LineEnding::Unix)
        }
    }
}

pub struct LineInfo {
    pub index: Arc<LineIndex>,
    pub ending: LineEnding,
    pub encoding: PositionEncoding,
}
