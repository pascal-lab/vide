use std::ops::Range;

/// A source buffer known to Slang while parsing a syntax tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBufferId {
    pub path: String,
    pub text: Option<String>,
    pub buffer_id: u32,
    pub origin: SourceBufferOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceBufferOrigin {
    Source,
    Predefine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBufferRange {
    pub buffer_id: u32,
    pub range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxTreeBufferIds {
    pub root_buffer_id: u32,
    pub source_buffers: Vec<SourceBufferId>,
}

/// A source location inside a Slang source buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceLocation {
    buffer_id: u32,
    offset: usize,
}

/// A half-open source range inside one or two Slang source buffers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceRange {
    start: SourceLocation,
    end: SourceLocation,
}

impl SourceLocation {
    pub(crate) fn from_parts(buffer_id: u32, offset: usize) -> Self {
        Self { buffer_id, offset }
    }

    pub fn buffer_id(self) -> u32 {
        self.buffer_id
    }

    pub fn offset(self) -> usize {
        self.offset
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
