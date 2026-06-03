use std::{fmt, hash};

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
            TimeUnit::Seconds => f.write_str("s"),
            TimeUnit::Milliseconds => f.write_str("ms"),
            TimeUnit::Microseconds => f.write_str("us"),
            TimeUnit::Nanoseconds => f.write_str("ns"),
            TimeUnit::Picoseconds => f.write_str("ps"),
            TimeUnit::Femtoseconds => f.write_str("fs"),
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
            Bit::L => f.write_str("0"),
            Bit::H => f.write_str("1"),
            Bit::X => f.write_str("x"),
            Bit::Z => f.write_str("z"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SVLogic {
    bit: Bit,
}

impl SVLogic {
    pub fn new(bit: Bit) -> Self {
        Self { bit }
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self.bit, Bit::X | Bit::Z)
    }

    pub fn bit(&self) -> Bit {
        self.bit
    }
}

#[derive(Clone, Eq)]
pub struct SVInt {
    value: u128,
    width: usize,
    signed: bool,
    has_unknown: bool,
}

impl SVInt {
    pub fn new(value: u128, width: usize, signed: bool, has_unknown: bool) -> Self {
        Self { value, width: width.max(1), signed, has_unknown }
    }

    pub fn from_literal(raw: &str) -> Option<Self> {
        parse_sv_int(raw)
    }

    pub fn is_signed(&self) -> bool {
        self.signed
    }

    pub fn has_unknown(&self) -> bool {
        self.has_unknown
    }

    pub fn get_bit_width(&self) -> usize {
        self.width
    }

    pub fn is_single_word(&self) -> bool {
        self.width <= u64::BITS as usize && !self.has_unknown
    }

    pub fn get_single_word(&self) -> Option<u64> {
        self.is_single_word().then_some(self.value as u64)
    }

    pub fn logic_eq(&self, other: &SVInt) -> SVLogic {
        SVLogic::new(if self == other { Bit::H } else { Bit::L })
    }

    pub fn serialize(&self, base: usize) -> String {
        if self.has_unknown {
            return "x".to_owned();
        }

        match base {
            2 => format!("{:b}", self.value),
            8 => format!("{:o}", self.value),
            10 => self.value.to_string(),
            16 => format!("{:x}", self.value),
            _ => self.value.to_string(),
        }
    }
}

impl fmt::Debug for SVInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SVInt")
            .field("value", &self.serialize(10))
            .field("width", &self.width)
            .field("signed", &self.signed)
            .field("has_unknown", &self.has_unknown)
            .finish()
    }
}

impl fmt::Display for SVInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.serialize(10))
    }
}

impl PartialEq for SVInt {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
            && self.width == other.width
            && self.signed == other.signed
            && self.has_unknown == other.has_unknown
    }
}

impl hash::Hash for SVInt {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.value.hash(state);
        self.width.hash(state);
        self.signed.hash(state);
        self.has_unknown.hash(state);
    }
}

fn parse_sv_int(raw: &str) -> Option<SVInt> {
    let cleaned: String = raw.chars().filter(|ch| *ch != '_').collect();
    let Some((size, rest)) = cleaned.split_once('\'') else {
        let value = cleaned.parse::<u128>().ok()?;
        let width = value_width(value).max(32);
        return Some(SVInt::new(value, width, false, false));
    };

    let width = size.parse::<usize>().ok().filter(|width| *width > 0).unwrap_or(32);
    let mut chars = rest.chars();
    let signed = matches!(chars.clone().next(), Some('s' | 'S'));
    if signed {
        chars.next();
    }
    let base = match chars.next()? {
        'b' | 'B' => 2,
        'o' | 'O' => 8,
        'd' | 'D' => 10,
        'h' | 'H' => 16,
        _ => return None,
    };
    let digits: String = chars.collect();
    let has_unknown = digits.chars().any(|ch| matches!(ch, 'x' | 'X' | 'z' | 'Z' | '?'));
    let normalized = digits
        .chars()
        .map(|ch| if matches!(ch, 'x' | 'X' | 'z' | 'Z' | '?') { '0' } else { ch })
        .collect::<String>();
    let value = u128::from_str_radix(normalized.trim_start_matches('+'), base).ok()?;
    Some(SVInt::new(value, width, signed, has_unknown))
}

fn value_width(value: u128) -> usize {
    (u128::BITS - value.leading_zeros()).max(1) as usize
}
