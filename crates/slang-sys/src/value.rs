use std::fmt;

use crate::syntax::ffi;
use tracing::warn;

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
    pub(crate) fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::Seconds),
            1 => Some(Self::Milliseconds),
            2 => Some(Self::Microseconds),
            3 => Some(Self::Nanoseconds),
            4 => Some(Self::Picoseconds),
            5 => Some(Self::Femtoseconds),
            raw => {
                warn!(raw, "Slang returned an unknown time unit; dropping the value");
                None
            }
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

    pub(crate) fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::L),
            1 => Some(Self::H),
            128 => Some(Self::X),
            64 => Some(Self::Z),
            raw => {
                warn!(raw, "Slang returned an unknown four-state bit; dropping the value");
                None
            }
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

#[cfg(test)]
mod tests {
    use super::{Bit, TimeUnit};

    #[test]
    fn unknown_time_unit_is_reported_as_absent() {
        assert_eq!(TimeUnit::from_raw(u8::MAX), None);
    }

    #[test]
    fn unknown_bit_is_reported_as_absent() {
        assert_eq!(Bit::from_raw(1), Some(Bit::H));
        assert_eq!(Bit::from_raw(u8::MAX), None);
    }
}
