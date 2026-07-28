/// A source location inside a Slang source buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceLocation {
    pub(crate) buffer_id: u32,
    pub(crate) offset: usize,
}

/// A half-open source range inside one or two Slang source buffers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceRange {
    pub(crate) start: SourceLocation,
    pub(crate) end: SourceLocation,
}

impl SourceLocation {
    pub fn buffer_id(self) -> Option<u32> {
        Some(self.buffer_id)
    }

    pub fn offset(self) -> Option<usize> {
        Some(self.offset)
    }
}

impl SourceRange {
    pub(crate) fn from_parts(
        start_buffer_id: u32,
        start_offset: usize,
        end_buffer_id: u32,
        end_offset: usize,
    ) -> Self {
        Self {
            start: SourceLocation { buffer_id: start_buffer_id, offset: start_offset },
            end: SourceLocation { buffer_id: end_buffer_id, offset: end_offset },
        }
    }

    pub fn start(self) -> usize {
        self.start.offset
    }

    pub fn end(self) -> usize {
        self.end.offset
    }

    pub fn start_buffer_id(self) -> u32 {
        self.start.buffer_id
    }

    pub fn end_buffer_id(self) -> u32 {
        self.end.buffer_id
    }

    pub fn is_single_buffer(self) -> bool {
        self.start.buffer_id == self.end.buffer_id
    }
}
