use either::Either;
use slang::{
    ChildrenIter, SyntaxToken, SyntaxTokenWithParent, SyntaxTrivia, Token, TokenKind,
    ast::{self, AstNode},
};
use utils::line_index::{TextRange, TextSize};

use crate::support;

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
}

impl<'a> SyntaxTokenWithParentExt<'a> for SyntaxTokenWithParent<'a> {
    #[inline]
    fn is_word_like(&self) -> bool {
        match self.kind() {
            TokenKind::IDENTIFIER | TokenKind::SYSTEM_IDENTIFIER => true,
            _ => is_word_like_value_text(&self.tok.value_text().to_string()),
        }
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

/// [`Either::Left`] represents the beg-token, and [`Either::Right`] represents
/// the end-token.
pub fn pair_token(
    SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<Either<SyntaxToken, SyntaxToken>> {
    let kind = tok.kind();

    macro_rules! P {
        ($beg:ident | $end:ident, $($rest:tt)*) => {
            if kind == Token![$beg] {
                Either::Right(support::child_token(parent, Token![$end])?)
            } else if kind == Token![$end] {
                Either::Left(support::child_token(parent, Token![$beg])?)
            } else {
                P! { $($rest)* }
            }
        };
        () => { return None; };
    }

    let res = match kind {
        Token![module] => {
            // move from header to declaration
            let parent = ast::ModuleDeclaration::cast(parent.parent().unwrap()).unwrap();
            Either::Right(parent.endmodule()?)
        }
        Token![endmodule] => {
            // move from declaration to header
            let parent = ast::ModuleDeclaration::cast(parent).unwrap();
            Either::Left(parent.header().module_keyword()?)
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
    fn trivias_with_range(
        &self,
    ) -> impl ChildrenIter<(TextRange, SyntaxTrivia<'a>)> + use<'a, Self>;
}

impl<'a> SyntaxTokenExt<'a> for SyntaxToken<'a> {
    #[inline]
    fn trivias_with_range(&self) -> impl ChildrenIter<(TextRange, SyntaxTrivia<'a>)> + use<'a> {
        self.trivias_with_loc().map(|((start, end), trivia)| {
            let range = TextRange::new(TextSize::new(start as u32), TextSize::new(end as u32));
            (range, trivia)
        })
    }
}
