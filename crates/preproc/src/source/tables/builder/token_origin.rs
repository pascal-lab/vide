use syntax::SourceBufferRange;
use utils::line_index::{TextRange, TextSize};

use super::*;

pub(super) fn source_range_from_origin(range: &SourceBufferRange) -> Option<SourceRange> {
    Some(SourceRange {
        source: PreprocSourceId::from(range.buffer_id),
        range: TextRange::new(
            TextSize::from(u32::try_from(range.range.start).ok()?),
            TextSize::from(u32::try_from(range.range.end).ok()?),
        ),
    })
}

pub(super) fn origin_index(index: u32) -> Option<usize> {
    usize::try_from(index).ok()
}
