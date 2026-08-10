use std::fmt;

use crate::syntax::ffi;

/// An owned four-state SystemVerilog integer value.
///
/// Slang calculates the value in C++; Rust receives stable scalar metadata and
/// textual representations, so no C++-owned arbitrary-precision object leaks
/// through the public API.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SVInt {
    bit_width: usize,
    is_signed: bool,
    has_unknown: bool,
    single_word: Option<u64>,
    binary: String,
    octal: String,
    decimal: String,
    hexadecimal: String,
}

impl SVInt {
    pub(crate) fn from_raw(raw: ffi::RawSVInt) -> Self {
        Self {
            bit_width: raw.bit_width as usize,
            is_signed: raw.is_signed,
            has_unknown: raw.has_unknown,
            single_word: raw.has_single_word.then_some(raw.single_word),
            binary: raw.binary,
            octal: raw.octal,
            decimal: raw.decimal,
            hexadecimal: raw.hexadecimal,
        }
    }

    pub fn is_signed(&self) -> bool { self.is_signed }
    pub fn has_unknown(&self) -> bool { self.has_unknown }
    pub fn get_bit_width(&self) -> usize { self.bit_width }
    pub fn is_single_word(&self) -> bool { self.single_word.is_some() }
    pub fn get_single_word(&self) -> Option<u64> { self.single_word }

    pub fn serialize(&self, base: usize) -> String {
        match base {
            2 => self.binary.clone(),
            8 => self.octal.clone(),
            10 => self.decimal.clone(),
            16 => self.hexadecimal.clone(),
            _ => panic!("unsupported SVInt serialization base: {base}"),
        }
    }
}

impl fmt::Display for SVInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.decimal) }
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

impl TimeUnit {
    pub(crate) fn from_raw(raw: u8) -> Self {
        match raw {
            0 => Self::Seconds,
            1 => Self::Milliseconds,
            2 => Self::Microseconds,
            3 => Self::Nanoseconds,
            4 => Self::Picoseconds,
            5 => Self::Femtoseconds,
            _ => panic!("unknown Slang time unit value: {raw}"),
        }
    }
}

impl fmt::Display for TimeUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Seconds => "s",
            Self::Milliseconds => "ms",
            Self::Microseconds => "us",
            Self::Nanoseconds => "ns",
            Self::Picoseconds => "ps",
            Self::Femtoseconds => "fs",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bit {
    L,
    H,
    X,
    Z,
}

impl Bit {
    pub fn bit(self) -> Self { self }

    pub(crate) fn from_raw(raw: u8) -> Self {
        match raw {
            0 => Self::L,
            1 => Self::H,
            128 => Self::X,
            64 => Self::Z,
            _ => panic!("unknown Slang four-state bit value: {raw}"),
        }
    }
}

impl fmt::Display for Bit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::L => "0",
            Self::H => "1",
            Self::X => "x",
            Self::Z => "z",
        })
    }
}
