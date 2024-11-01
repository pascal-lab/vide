use either::Either;
use slang::{
    SyntaxToken, SyntaxTokenWithParent, T, TokenKind,
    ast::{self, AstNode},
};

use crate::support;

pub trait TokenKindExt {
    fn is_pair_token(self) -> bool;
}

impl TokenKindExt for TokenKind {
    fn is_pair_token(self) -> bool {
        macro_rules! P {
        ($($tok:ident)|* $(|)?) => {
            $(self == T![$tok] ||)* false
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
}

/// [`Either::Left`] represents the beg-token, and [`Either::Right`] represents
/// the end-token.
pub fn pair_token(
    SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<Either<SyntaxToken, SyntaxToken>> {
    let kind = tok.kind();

    macro_rules! P {
        ($beg:ident | $end:ident, $($rest:tt)*) => {
            if kind == T![$beg] {
                Either::Right(support::child_token(parent, T![$end])?)
            } else if kind == T![$end] {
                Either::Left(support::child_token(parent, T![$beg])?)
            } else {
                P! { $($rest)* }
            }
        };
        () => { return None; };
    }

    let res = match kind {
        T![module] => {
            // move from header to declaration
            let parent = ast::ModuleDeclaration::cast(parent.parent().unwrap()).unwrap();
            Either::Right(parent.endmodule()?)
        }
        T![endmodule] => {
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
