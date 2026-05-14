use std::fmt;

use syntax::{Bit, SVInt, TimeUnit, ast};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Literal {
    Int(SVInt),
    Float(FloatTypeWrapper),
    Time { val: FloatTypeWrapper, unit: TimeUnit },
    Str(Box<str>),
    UnbasedUnsized(Bit),
}

#[derive(Default, Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct FloatTypeWrapper(u64);

impl FloatTypeWrapper {
    fn new(value: f64) -> Self {
        Self(value.to_bits())
    }

    pub fn to_bits(self) -> u64 {
        self.0
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

pub(crate) fn lower_literal(literal: ast::LiteralExpression) -> Option<Literal> {
    use ast::LiteralExpression::*;
    match literal {
        UnbasedUnsizedLiteralExpression(syntax_node) => {
            let bit = syntax_node.child_token(0)?.bits()?;
            Some(Literal::UnbasedUnsized(bit.bit()))
        }
        RealLiteralExpression(syntax_node) => {
            let real = syntax_node.child_token(0)?.real()?;
            Some(Literal::Float(FloatTypeWrapper::new(real)))
        }
        TimeLiteralExpression(syntax_node) => {
            let time = syntax_node.child_token(0)?;
            let val = FloatTypeWrapper::new(time.real()?);
            let unit = time.time_unit()?;
            Some(Literal::Time { val, unit })
        }
        IntegerLiteralExpression(syntax_node) => {
            let int = syntax_node.child_token(0)?.int()?;
            Some(Literal::Int(int))
        }
        StringLiteralExpression(syntax_node) => {
            let s = syntax_node.child_token(0)?.value_text().to_string();
            Some(Literal::Str(s.into_boxed_str()))
        }
        NullLiteralExpression(_)
        | WildcardLiteralExpression(_)
        | DefaultPatternKeyExpression(_) => None,
    }
}

pub(crate) fn lower_integer_vector(int_vec: ast::IntegerVectorExpression) -> Option<Literal> {
    Some(Literal::Int(int_vec.value()?.int()?))
}
