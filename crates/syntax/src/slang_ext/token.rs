use either::Either;
use slang::{
    ChildrenIter, LiteralBase, SyntaxNode, SyntaxToken, SyntaxTokenWithParent, SyntaxTrivia, Token,
    TokenKind,
    ast::{self, AstNode},
};
use utils::line_index::{TextRange, TextSize};

use crate::{SyntaxNodeExt, support};

pub trait TokenKindExt {
    fn is_pair_token(&self) -> bool;
    fn name_like(&self) -> bool;
    fn is_literal(&self) -> bool;
}

impl TokenKindExt for TokenKind {
    #[inline]
    fn is_pair_token(&self) -> bool {
        macro_rules! P {
        ($($tok:ident)|* $(|)?) => {
            $(*self == Token![$tok] ||)* false
        };
    }
        P! {
            begin | end
            | module | endmodule
            | case | endcase
            | function | endfunction
            | generate | endgenerate
            | interface | endinterface
            | task | endtask
        }
    }

    #[inline]
    fn name_like(&self) -> bool {
        matches!(*self, TokenKind::IDENTIFIER | TokenKind::SYSTEM_IDENTIFIER)
    }

    #[inline]
    fn is_literal(&self) -> bool {
        matches!(
            *self,
            TokenKind::INTEGER_LITERAL
                | TokenKind::INTEGER_BASE
                | TokenKind::REAL_LITERAL
                | TokenKind::STRING_LITERAL
                | TokenKind::UNBASED_UNSIZED_LITERAL
                | TokenKind::TIME_LITERAL
        )
    }
}

pub trait SyntaxTokenWithParentExt<'a> {
    fn is_word_like(&self) -> bool;
    fn trivias_with_range(
        &self,
    ) -> impl ChildrenIter<(TextRange, SyntaxTrivia<'a>)> + use<'a, Self>;
}

impl<'a> SyntaxTokenWithParentExt<'a> for SyntaxTokenWithParent<'a> {
    #[inline]
    fn is_word_like(&self) -> bool {
        match self.kind() {
            TokenKind::IDENTIFIER | TokenKind::SYSTEM_IDENTIFIER => true,
            _ => is_word_like_value_text(&self.tok.value_text().to_string()),
        }
    }

    #[inline]
    fn trivias_with_range(&self) -> impl ChildrenIter<(TextRange, SyntaxTrivia<'a>)> + use<'a> {
        self.tok.trivias_with_range_in_root(self.parent.find_root())
    }
}

#[inline]
fn is_word_like_value_text(text: &str) -> bool {
    fn is_ident_start(b: u8) -> bool {
        matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'_')
    }

    fn is_ident_continue(b: u8) -> bool {
        is_ident_start(b) || matches!(b, b'0'..=b'9' | b'$')
    }

    let bytes = text.as_bytes();
    let Some((&first, rest)) = bytes.split_first() else {
        return false;
    };

    if first == b'$' {
        let Some((&second, rest)) = rest.split_first() else {
            return false;
        };
        is_ident_start(second) && rest.iter().copied().all(is_ident_continue)
    } else {
        is_ident_start(first) && rest.iter().copied().all(is_ident_continue)
    }
}

pub fn integer_literal_base_specifier_candidates() -> Vec<String> {
    literal_bases()
        .into_iter()
        .flat_map(|base| {
            let specifier = integer_literal_base_specifier(base);
            [specifier.to_owned(), format!("s{specifier}")]
        })
        .collect()
}

fn literal_bases() -> [LiteralBase; 4] {
    [LiteralBase::Bin, LiteralBase::Oct, LiteralBase::Dec, LiteralBase::Hex]
}

fn integer_literal_base_specifier(base: LiteralBase) -> &'static str {
    match base {
        LiteralBase::Bin => "b",
        LiteralBase::Oct => "o",
        LiteralBase::Dec => "d",
        LiteralBase::Hex => "h",
    }
}

/// [`Either::Left`] represents the beg-token, and [`Either::Right`] represents
/// the end-token.
pub fn pair_token(
    SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<Either<SyntaxTokenWithParent, SyntaxTokenWithParent>> {
    let kind = tok.kind();

    macro_rules! P {
        ($beg:ident | $end:ident, $($rest:tt)*) => {
            if kind == Token![$beg] {
                Either::Right(SyntaxTokenWithParent {
                    parent,
                    tok: support::child_token(parent, Token![$end])?,
                })
            } else if kind == Token![$end] {
                Either::Left(SyntaxTokenWithParent {
                    parent,
                    tok: support::child_token(parent, Token![$beg])?,
                })
            } else {
                P! { $($rest)* }
            }
        };
        () => { return None; };
    }

    let res = match kind {
        Token![module] => {
            // move from header to declaration
            let decl = ast::ModuleDeclaration::cast(parent.parent()?)?;
            Either::Right(SyntaxTokenWithParent { parent: decl.syntax(), tok: decl.endmodule()? })
        }
        Token![endmodule] => {
            // move from declaration to header
            let decl = ast::ModuleDeclaration::cast(parent)?;
            let header = decl.header();
            Either::Left(SyntaxTokenWithParent {
                parent: header.syntax(),
                tok: header.module_keyword()?,
            })
        }
        _ => {
            P! {
                begin | end,
                case | endcase,
                function | endfunction,
                generate | endgenerate,
                interface | endinterface,
                task | endtask,
            }
        }
    };

    Some(res)
}

pub trait SyntaxTokenExt<'a> {
    fn trivias_with_range_in_root(
        &self,
        root: SyntaxNode<'a>,
    ) -> impl ChildrenIter<(TextRange, SyntaxTrivia<'a>)> + use<'a, Self>;
}

impl<'a> SyntaxTokenExt<'a> for SyntaxToken<'a> {
    #[inline]
    fn trivias_with_range_in_root(
        &self,
        root: SyntaxNode<'a>,
    ) -> impl ChildrenIter<(TextRange, SyntaxTrivia<'a>)> + use<'a> {
        let Some(root_range) = root.range().filter(|range| range.is_single_buffer()) else {
            return Either::Left(std::iter::empty());
        };
        let root_buffer_id = root_range.start_buffer_id();

        let trivias = self
            .trivias_with_loc()
            .filter_map(move |(loc, trivia)| {
                if loc.buffer_id != root_buffer_id {
                    return None;
                }
                let start = u32::try_from(loc.start).ok()?;
                let end = u32::try_from(loc.end).ok()?;
                Some((TextRange::new(TextSize::new(start), TextSize::new(end)), trivia))
            })
            .collect::<Vec<_>>();
        Either::Right(trivias.into_iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_literal_base_specifier_candidates_follow_slang_literal_bases() {
        let mut candidates = integer_literal_base_specifier_candidates();
        candidates.sort();

        assert_eq!(candidates, ["b", "d", "h", "o", "sb", "sd", "sh", "so"]);
    }
}
