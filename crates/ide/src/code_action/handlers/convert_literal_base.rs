use syntax::{
    LiteralBase, SVInt,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};
use utils::text_edit::TextRange;

use crate::code_action::{CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind};

const ACTION_ID: CodeActionId = CodeActionId {
    name: "convert_literal_base",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};

pub(super) fn convert_literal_base(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let literal = SelectedIntegerLiteral::from_context(ctx)?;

    for target_base in literal.target_bases() {
        let range = literal.range;
        let label = target_base.action_label();
        let replacement = literal.render(target_base);
        collector.add(ACTION_ID, label, range, |builder| {
            builder.replace(range, replacement);
        });
    }

    Some(())
}

#[derive(Debug)]
struct SelectedIntegerLiteral {
    range: TextRange,
    value: SVInt,
    notation: LiteralNotation,
}

impl SelectedIntegerLiteral {
    fn from_context(ctx: &CodeActionCtx) -> Option<Self> {
        ctx.find_node_at_offset::<ast::IntegerVectorExpression>()
            .and_then(Self::from_based_syntax)
            .or_else(|| {
                ctx.find_node_at_offset::<ast::LiteralExpression>()
                    .and_then(Self::from_plain_decimal_syntax)
            })
    }

    fn from_plain_decimal_syntax(literal: ast::LiteralExpression) -> Option<Self> {
        let ast::LiteralExpression::IntegerLiteralExpression(integer) = literal else {
            return None;
        };

        let token = integer.child_token(0)?;
        Some(Self {
            range: integer.text_range()?,
            value: token.int()?,
            notation: LiteralNotation::plain_decimal(),
        })
    }

    fn from_based_syntax(literal: ast::IntegerVectorExpression) -> Option<Self> {
        let value = literal.value()?;
        Some(Self {
            range: literal.syntax().text_range()?,
            value: value.int()?,
            notation: LiteralNotation::based(literal)?,
        })
    }

    fn target_bases(&self) -> impl Iterator<Item = LiteralRadix> + '_ {
        LiteralRadix::ALL.into_iter().filter(|base| *base != self.notation.base)
    }

    fn render(&self, target_base: LiteralRadix) -> String {
        self.notation.render(&self.value, target_base)
    }
}

#[derive(Debug)]
struct LiteralNotation {
    base: LiteralRadix,
    width: Option<String>,
    signed: bool,
}

impl LiteralNotation {
    fn plain_decimal() -> Self {
        Self { base: LiteralRadix::Decimal, width: None, signed: false }
    }

    fn based(literal: ast::IntegerVectorExpression) -> Option<Self> {
        let base = literal.base()?;
        Some(Self {
            base: LiteralRadix::from(base.base()?),
            width: literal.size().map(|size| size.raw_text().to_string()),
            signed: base.raw_text().as_bytes().iter().any(|byte| byte.eq_ignore_ascii_case(&b's')),
        })
    }

    fn render(&self, value: &SVInt, target_base: LiteralRadix) -> String {
        let digits = value.serialize(target_base.radix());
        if self.can_render_as_plain_decimal(target_base) {
            return digits;
        }

        let mut text = String::new();
        if let Some(width) = &self.width {
            text.push_str(width);
        }
        text.push('\'');
        if self.signed {
            text.push('s');
        }
        text.push_str(target_base.specifier());
        text.push_str(&digits);
        text
    }

    fn can_render_as_plain_decimal(&self, target_base: LiteralRadix) -> bool {
        target_base == LiteralRadix::Decimal && self.width.is_none() && !self.signed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LiteralRadix {
    Binary,
    Octal,
    Decimal,
    Hexadecimal,
}

impl LiteralRadix {
    const ALL: [Self; 4] = [Self::Binary, Self::Octal, Self::Decimal, Self::Hexadecimal];

    fn radix(self) -> usize {
        match self {
            Self::Binary => 2,
            Self::Octal => 8,
            Self::Decimal => 10,
            Self::Hexadecimal => 16,
        }
    }

    fn specifier(self) -> &'static str {
        match self {
            Self::Binary => "b",
            Self::Octal => "o",
            Self::Decimal => "d",
            Self::Hexadecimal => "h",
        }
    }

    fn action_label(self) -> &'static str {
        match self {
            Self::Binary => "Convert literal to binary",
            Self::Octal => "Convert literal to octal",
            Self::Decimal => "Convert literal to decimal",
            Self::Hexadecimal => "Convert literal to hexadecimal",
        }
    }
}

impl From<LiteralBase> for LiteralRadix {
    fn from(base: LiteralBase) -> Self {
        match base {
            LiteralBase::Bin => Self::Binary,
            LiteralBase::Oct => Self::Octal,
            LiteralBase::Dec => Self::Decimal,
            LiteralBase::Hex => Self::Hexadecimal,
        }
    }
}
