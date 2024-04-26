use std::{fmt, iter};

use smallvec::SmallVec;
use syntax::ast::{self, AstNode};

use super::{
    bit::{Bit, BoxBasedBitVector},
    lower::Lower,
};

use crate::try_match;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum Literal {
    // 2 states
    Int(i64),
    Float(FloatTypeWrapper),
    // TODO: optimization for U64BasedBitVector
    // 4 states
    Vector { bits: BoxBasedBitVector, signed: bool, base: Base },
    Time(TimeLiteral),
    Str(Box<str>),
    UnbasedUnsized(Bit),
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct TimeLiteral {
    // TODO: support float time
    v: usize,
    unit: TimeUnit,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum TimeUnit {
    S,
    Ms,
    Us,
    Ns,
    Ps,
    Fs,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum Base {
    Bin,
    Oct,
    Dec,
    Hex,
}

// FloatTypeWrapper is a wrapper around f64 to allow it to be used in a HashMap.
#[derive(Default, Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct FloatTypeWrapper(u64);

impl FloatTypeWrapper {
    pub fn new(value: f64) -> Self {
        Self(value.to_bits())
    }
}

impl From<f64> for FloatTypeWrapper {
    fn from(value: f64) -> Self {
        Self::new(value)
    }
}

impl From<f32> for FloatTypeWrapper {
    fn from(value: f32) -> Self {
        Self::new(value as f64)
    }
}

impl From<FloatTypeWrapper> for f64 {
    fn from(val: FloatTypeWrapper) -> Self {
        f64::from_bits(val.0)
    }
}

impl From<FloatTypeWrapper> for f32 {
    fn from(val: FloatTypeWrapper) -> Self {
        f64::from_bits(val.0) as f32
    }
}

impl fmt::Display for FloatTypeWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", f64::from_bits(self.0))
    }
}

pub(crate) trait LowerLiteral: Lower {
    fn lower_literal(&self, literal: &ast::PrimaryLiteral) -> Option<Literal> {
        try_match! {
            literal.number(), num => self.lower_number(&num),
            literal.time_literal(), time => self.lower_time_literal(&time).map(Literal::Time),
            literal.unbased_unsized_literal(), uu => try_match! {
                uu.token_single_quote_0(), _ => Some(Literal::UnbasedUnsized(Bit::L)),
                uu.token_single_quote_1(), _ => Some(Literal::UnbasedUnsized(Bit::H)),
                _ => None
            },
            _ => None,
        }
    }

    fn lower_unsigned_number(&self, un: &ast::UnsignedNumber) -> Option<Literal> {
        let text = self.file_text();
        let v = un.to_text(text).and_then(|v| v.parse::<i64>().ok())?;
        Some(Literal::Int(v))
    }

    fn lower_number(&self, number: &ast::Number) -> Option<Literal> {
        fn lower_int_num(text: &str, wid: u8, base: Base, b1: char, b2: char) -> Option<Literal> {
            let mut len = 0;
            let mut bits = vec![];
            let mut has_met_base = false;
            let mut signed = false;
            for c in text.chars() {
                match c {
                    's' | 'S' => signed = true,
                    c if c == b1 || c == b2 => {
                        has_met_base = true;
                        bits.reserve(len);
                    }
                    '0'..='9' if !has_met_base => len = len * 10 + (c.to_digit(10)? as usize),
                    c @ ('x' | 'X' | 'z' | 'Z' | '?') if has_met_base => {
                        bits.extend(iter::repeat(c.into()).take(wid.into()));
                    }
                    c @ ('0'..='9' | 'a'..='f' | 'A'..='F') if has_met_base => {
                        let c = c.to_digit(16)? as u8;
                        (0..wid).rev().for_each(|i| bits.push((((c >> i) & 1) != 0).into()));
                    }
                    '_' | '\'' => {}
                    _ => unreachable!(),
                }
            }
            assert!(has_met_base);
            bits.reverse();
            if bits.len() < len {
                bits.extend(iter::repeat(Bit::L).take(len - bits.len()));
            }

            let bits = BoxBasedBitVector { bits: bits.into_boxed_slice() };
            Some(Literal::Vector { bits, signed, base })
        }

        try_match!(
            number.integral_number(), i => {
                let text = number.to_text(self.file_text())?;
                try_match! {
                    i.binary_number(), _ => lower_int_num(text, 1, Base::Bin, 'b', 'B'),
                    i.octal_number(), _ => lower_int_num(text, 3, Base::Oct, 'o', 'O'),
                    i.hex_number(), _ => lower_int_num(text, 4, Base::Hex, 'h', 'H'),
                    i.decimal_number(), b => try_match!(
                        b.unsigned_number(), un => self.lower_unsigned_number(&un),
                        _ => {
                            let (len, res) = text.split_once('\'').unwrap();
                            let len = len.parse::<usize>().ok().unwrap();
                            let signed = res.starts_with('s') || res.starts_with('S');
                            assert!(res.chars().nth( if signed { 1 } else {0} ) == Some('d') ||
                                    res.chars().nth( if signed { 1 } else {0} ) == Some('D'));
                            let bits = {
                                let start = if signed { 2 } else { 1 };
                                let sym = res.chars().nth(start);
                                match sym {
                                    Some('x' | 'X') => iter::repeat(Bit::X).take(len).collect(),
                                    Some('z' | 'Z' | '?') => iter::repeat(Bit::Z).take(len).collect(),
                                    Some('0'..='9') => {
                                        let digits = res
                                            .chars()
                                            .skip(start)
                                            .collect::<String>()
                                            .parse::<u64>()
                                            .unwrap();
                                        let mut bits: SmallVec<[Bit; 16]> = (0..len)
                                            .map(|i| ((digits >> i) & 1 != 0).into())
                                            .collect();
                                        if bits.len() < len {
                                            bits.extend(iter::repeat(Bit::L).take(len - bits.len()));
                                        }
                                        bits
                                    }
                                    _ => unreachable!()
                                }
                            };

                            let bits = BoxBasedBitVector { bits: bits.into_boxed_slice() };
                            Some(Literal::Vector { bits, signed, base: Base::Dec })
                        }
                    ),
                    _ => unreachable!()
                }
            },
            _ => unimplemented!()
        )
    }

    fn lower_time_literal(&self, time: &ast::TimeLiteral) -> Option<TimeLiteral> {
        fn lower_time_unit(unit: &ast::TimeUnit) -> Option<TimeUnit> {
            macro_rules! match_unit {
                ($u:ident, $e:ident) => {
                    if unit.$u().is_some() {
                        return Some(TimeUnit::$e);
                    }
                };
            }
            match_unit!(token_s, S);
            match_unit!(token_ms, Ms);
            match_unit!(token_us, Us);
            match_unit!(token_ns, Ns);
            match_unit!(token_ps, Ps);
            match_unit!(token_fs, Fs);
            None
        }
        let text = || self.file_text();

        let unit = lower_time_unit(&time.time_unit()?)?;

        try_match!(time.fixed_point_number(), _ => unimplemented!());
        try_match!(time.unsigned_number(), un => {
            let v = un.to_text(text()).and_then(|v| v.parse::<usize>().ok())?;
            return Some(TimeLiteral { v, unit });
        });

        None
    }

    fn lower_delay_value(&mut self, delay_value: &ast::DelayValue) -> Option<Literal> {
        try_match! {
            delay_value.unsigned_number(), un => self.lower_unsigned_number(&un),
            delay_value.time_literal(), time => Some(Literal::Time(self.lower_time_literal(&time)?)),
            _ => {
                return None;
                todo!("Unsupported");
            }
        }
    }
}
