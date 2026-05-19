use syntax::{
    LiteralBase, SVInt,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};
use utils::text_edit::TextRange;

use crate::code_action::{CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind};

const ID: CodeActionId = CodeActionId {
    name: "convert_literal_base",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};

pub(super) fn convert_literal_base(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let literal = literal_at(ctx)?;

    for target_base in IntegerBase::ALL {
        if target_base == literal.base {
            continue;
        }

        let Some(replacement) = literal.render(target_base) else {
            continue;
        };
        let label = format!("Convert literal to {}", target_base.label());
        collector.add(ID, label, literal.range, |builder| {
            builder.replace(literal.range, replacement);
        });
    }

    Some(())
}

#[derive(Debug)]
struct IntegerLiteral {
    range: TextRange,
    value: SVInt,
    base: IntegerBase,
    notation: IntegerLiteralNotation,
}

impl IntegerLiteral {
    fn render(&self, base: IntegerBase) -> Option<String> {
        if base == IntegerBase::Dec && self.value.has_unknown() {
            return None;
        }

        let digits = self.value.serialize(base.radix());
        Some(match &self.notation {
            IntegerLiteralNotation::PlainDecimal => {
                format!("{}'s{}{}", self.plain_decimal_width(), base.specifier(), digits)
            }
            IntegerLiteralNotation::Based { size: Some(size), signed } => {
                format!("{size}'{}{}{}", signed_specifier(*signed), base.specifier(), digits)
            }
            IntegerLiteralNotation::Based { size: None, signed } => {
                format!("'{}{}{}", signed_specifier(*signed), base.specifier(), digits)
            }
        })
    }

    fn plain_decimal_width(&self) -> usize {
        let width = self.value.get_bit_width();
        if width < 32 {
            32
        } else if self.value.is_signed() {
            width
        } else {
            width + 1
        }
    }
}

#[derive(Debug)]
enum IntegerLiteralNotation {
    PlainDecimal,
    Based { size: Option<String>, signed: bool },
}

fn signed_specifier(signed: bool) -> &'static str {
    if signed { "s" } else { "" }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntegerBase {
    Bin,
    Oct,
    Dec,
    Hex,
}

impl IntegerBase {
    const ALL: [Self; 4] = [Self::Bin, Self::Oct, Self::Dec, Self::Hex];

    fn from_literal_base(base: LiteralBase) -> Self {
        match base {
            LiteralBase::Bin => Self::Bin,
            LiteralBase::Oct => Self::Oct,
            LiteralBase::Dec => Self::Dec,
            LiteralBase::Hex => Self::Hex,
        }
    }

    fn radix(self) -> usize {
        match self {
            Self::Bin => 2,
            Self::Oct => 8,
            Self::Dec => 10,
            Self::Hex => 16,
        }
    }

    fn specifier(self) -> &'static str {
        match self {
            Self::Bin => "b",
            Self::Oct => "o",
            Self::Dec => "d",
            Self::Hex => "h",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Bin => "binary",
            Self::Oct => "octal",
            Self::Dec => "decimal",
            Self::Hex => "hexadecimal",
        }
    }
}

fn literal_at(ctx: &CodeActionCtx) -> Option<IntegerLiteral> {
    if let Some(literal) =
        ctx.find_node_at_offset::<ast::IntegerVectorExpression>().and_then(integer_vector_literal)
    {
        return Some(literal);
    }

    let literal = ctx.find_node_at_offset::<ast::LiteralExpression>()?;
    let ast::LiteralExpression::IntegerLiteralExpression(integer) = literal else {
        return None;
    };

    let token = integer.child_token(0)?;
    Some(IntegerLiteral {
        range: integer.text_range()?,
        value: token.int()?,
        base: IntegerBase::Dec,
        notation: IntegerLiteralNotation::PlainDecimal,
    })
}

fn integer_vector_literal(literal: ast::IntegerVectorExpression) -> Option<IntegerLiteral> {
    let base = literal.base()?;
    let value = literal.value()?;
    Some(IntegerLiteral {
        range: literal.syntax().text_range()?,
        value: value.int()?,
        base: IntegerBase::from_literal_base(base.base()?),
        notation: IntegerLiteralNotation::Based {
            size: literal.size().map(|size| size.raw_text().to_string()),
            signed: base.raw_text().as_bytes().iter().any(|byte| byte.eq_ignore_ascii_case(&b's')),
        },
    })
}
