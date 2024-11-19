use either::Either;
use line_index::{TextRange, TextSize};
use slang::{
    SyntaxToken, SyntaxTokenWithParent, SyntaxTrivia, Token, TokenKind,
    ast::{self, AstNode},
};

use crate::support;

pub trait TokenKindExt {
    fn is_pair_token(&self) -> bool;
    fn name_like(&self) -> bool;
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
    ) -> Option<impl DoubleEndedIterator<Item = (TextRange, SyntaxTrivia<'a>)> + ExactSizeIterator>;
}

impl<'a> SyntaxTokenExt<'a> for SyntaxToken<'a> {
    #[inline]
    fn trivias_with_range(
        &self,
    ) -> Option<impl DoubleEndedIterator<Item = (TextRange, SyntaxTrivia<'a>)> + ExactSizeIterator>
    {
        let iter = self.trivias_with_loc()?.map(|((start, end), trivia)| {
            let range = TextRange::new(TextSize::new(start as u32), TextSize::new(end as u32));
            (range, trivia)
        });
        Some(iter)
    }
}
