use std::{
    ffi::c_char,
    fmt::{self, Display},
    hash,
    ops::Not,
};

use cxx::UniquePtr;

use crate::ffi;

pub struct SVInt {
    pub(crate) _ptr: UniquePtr<ffi::SVInt>,
}

pub struct SVLogic {
    pub(crate) _ptr: UniquePtr<ffi::SVLogic>,
}

pub struct SourceLocation {
    _ptr: UniquePtr<ffi::SourceLocation>,
}

pub struct SourceRange {
    _ptr: UniquePtr<ffi::SourceRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeUnit {
    Seconds,
    Milliseconds,
    Microseconds,
    Nanoseconds,
    Picoseconds,
    Femtoseconds,
}

impl fmt::Display for TimeUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimeUnit::Seconds => write!(f, "s"),
            TimeUnit::Milliseconds => write!(f, "ms"),
            TimeUnit::Microseconds => write!(f, "us"),
            TimeUnit::Nanoseconds => write!(f, "ns"),
            TimeUnit::Picoseconds => write!(f, "ps"),
            TimeUnit::Femtoseconds => write!(f, "fs"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LiteralBase {
    Bin,
    Oct,
    Dec,
    Hex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bit {
    L,
    H,
    X,
    Z,
}

impl fmt::Display for Bit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Bit::L => write!(f, "0"),
            Bit::H => write!(f, "1"),
            Bit::X => write!(f, "x"),
            Bit::Z => write!(f, "z"),
        }
    }
}

impl SourceLocation {
    #[cfg(target_pointer_width = "64")]
    const NO_LOCATION: usize = (1usize << 36) - 1;
    #[cfg(target_pointer_width = "32")]
    const NO_LOCATION: usize = usize::MAX;

    #[inline]
    pub fn from_unique_ptr(_ptr: UniquePtr<ffi::SourceLocation>) -> Option<Self> {
        _ptr.is_null().not().then(|| SourceLocation { _ptr })
    }

    #[inline]
    pub fn offset(&self) -> Option<usize> {
        let offset = self._ptr.offset();
        (offset == Self::NO_LOCATION).not().then_some(offset)
    }

    #[inline]
    pub fn buffer_id(&self) -> Option<u32> {
        self.offset().map(|_| self._ptr.buffer_id())
    }
}

impl fmt::Debug for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceLocation")
            .field("buffer_id", &self.buffer_id())
            .field("offset", &self.offset())
            .finish()
    }
}

impl PartialEq for SourceLocation {
    fn eq(&self, other: &Self) -> bool {
        self.buffer_id() == other.buffer_id() && self.offset() == other.offset()
    }
}

impl Eq for SourceLocation {}

impl SourceRange {
    #[inline]
    pub(crate) fn from_unique_ptr(_ptr: UniquePtr<ffi::SourceRange>) -> Option<Self> {
        _ptr.is_null().not().then(|| SourceRange { _ptr })
    }

    #[inline]
    pub fn start(&self) -> usize {
        self._ptr.start()
    }

    #[inline]
    pub fn end(&self) -> usize {
        self._ptr.end()
    }

    #[inline]
    pub fn start_buffer_id(&self) -> u32 {
        self._ptr.start_buffer_id()
    }

    #[inline]
    pub fn end_buffer_id(&self) -> u32 {
        self._ptr.end_buffer_id()
    }

    #[inline]
    pub fn is_single_buffer(&self) -> bool {
        self.start_buffer_id() == self.end_buffer_id()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start() >= self.end()
    }
}

impl fmt::Debug for SourceRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceRange")
            .field("start_buffer_id", &self.start_buffer_id())
            .field("start", &self.start())
            .field("end_buffer_id", &self.end_buffer_id())
            .field("end", &self.end())
            .finish()
    }
}

impl PartialEq for SourceRange {
    fn eq(&self, other: &Self) -> bool {
        self.start_buffer_id() == other.start_buffer_id()
            && self.start() == other.start()
            && self.end_buffer_id() == other.end_buffer_id()
            && self.end() == other.end()
    }
}

impl Eq for SourceRange {}

impl SVLogic {
    #[inline]
    pub fn is_unknown(&self) -> bool {
        self._ptr.isUnknown()
    }

    #[inline]
    pub fn char(&self) -> c_char {
        self._ptr.toChar()
    }

    #[inline]
    pub fn bit(&self) -> Bit {
        const X: u8 = 1 << 7;
        const Z: u8 = 1 << 6;
        match self._ptr.value() {
            0 => Bit::L,
            1 => Bit::H,
            X => Bit::X,
            Z => Bit::Z,
            _ => unreachable!(),
        }
    }
}

impl SVInt {
    #[inline]
    pub fn is_signed(&self) -> bool {
        self._ptr.isSigned()
    }

    #[inline]
    pub fn has_unknown(&self) -> bool {
        self._ptr.hasUnknown()
    }

    #[inline]
    pub fn get_bit_width(&self) -> usize {
        self._ptr.getBitWidth() as usize
    }

    #[inline]
    pub fn is_single_word(&self) -> bool {
        const CHAR_BIT: usize = core::ffi::c_char::BITS as usize;
        const BITS_PER_WORD: usize = core::mem::size_of::<u64>() * CHAR_BIT;
        self.get_bit_width() <= BITS_PER_WORD && !self.has_unknown()
    }

    #[inline]
    pub fn get_single_word(&self) -> Option<u64> {
        self.is_single_word().then(|| unsafe { *self._ptr.getRawPtr() })
    }

    #[inline]
    pub fn logic_eq(&self, other: &SVInt) -> SVLogic {
        let logic = self._ptr.eq(&other._ptr);
        SVLogic { _ptr: logic }
    }

    #[inline]
    pub fn serialize(&self, base: usize) -> String {
        self._ptr.toString(base)
    }
}

unsafe impl Send for SVInt {}

unsafe impl Sync for SVInt {}

impl fmt::Debug for SVInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SVInt").field("to_string", &self.to_string()).finish()
    }
}

impl Clone for SVInt {
    fn clone(&self) -> Self {
        SVInt { _ptr: self._ptr.clone() }
    }
}

impl PartialEq for SVInt {
    fn eq(&self, other: &Self) -> bool {
        let logic = self.logic_eq(other);
        logic.bit() == Bit::H
    }
}

impl Eq for SVInt {}

impl hash::Hash for SVInt {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self._ptr.getRawPtr().hash(state)
    }
}

impl Display for SVInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self._ptr.toString(10))
    }
}
