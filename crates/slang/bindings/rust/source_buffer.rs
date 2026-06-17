use std::ops::Range;

use crate::ffi;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxTreeBuffer {
    pub path: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxTreeBufferIds {
    pub root_buffer_id: u32,
    pub source_buffers: Vec<SourceBufferId>,
}

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

impl SourceBufferOrigin {
    pub(crate) fn from_raw(raw: u8) -> Self {
        match raw {
            0 => SourceBufferOrigin::Source,
            1 => SourceBufferOrigin::Predefine,
            origin => panic!("unexpected source buffer origin {origin}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBufferRange {
    pub buffer_id: u32,
    pub range: Range<usize>,
}

impl SyntaxTreeBufferIds {
    #[inline]
    pub(crate) fn from_raw(raw: ffi::RawSyntaxTreeBufferIds) -> Self {
        Self {
            root_buffer_id: raw.root_buffer_id,
            source_buffers: raw
                .source_buffers
                .into_iter()
                .map(|buffer| SourceBufferId {
                    path: buffer.path,
                    text: buffer.has_text.then_some(buffer.text),
                    buffer_id: buffer.buffer_id,
                    origin: SourceBufferOrigin::from_raw(buffer.origin),
                })
                .collect(),
        }
    }
}

impl SourceBufferRange {
    #[inline]
    pub(crate) fn from_raw(raw: ffi::RawSourceBufferRange) -> Option<Self> {
        raw.has_range
            .then_some(Self { buffer_id: raw.buffer_id, range: raw.range_start..raw.range_end })
    }
}
