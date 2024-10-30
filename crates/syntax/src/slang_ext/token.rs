use either::Either;
use slang::{TokenKind, T};

/// [`Either::Left`] represents the beg-token, and [`Either::Right`] represents the
/// end-token.
pub fn token_pair(kind: TokenKind) -> Option<Either<TokenKind, TokenKind>> {
    macro_rules! P {
        ($beg:ident, $end:ident; $($rest:tt)*) => {
            if kind == T![$beg] {
                Some(Either::Right(T![$end]))
            } else if kind == T![$end] {
                Some(Either::Left(T![$beg]))
            } else {
                P! { $($rest)* }
            }
        };
        () => { None };
    }
    P! {
        begin, end;
        case, endcase;
        function, endfunction;
        generate, endgenerate;
        interface, endinterface;
        module, endmodule;
        task, endtask;
    }
}
