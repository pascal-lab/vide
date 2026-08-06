//! Shared token-selection helpers for IDE features.
//!
//! Every feature that resolves a caret offset to a token picks between the
//! tokens straddling the offset with a precedence function. The three
//! precedence shapes below are the only ones in use; keeping them here as
//! named functions instead of per-feature copies makes the intent explicit
//! and the shapes auditable.

use syntax::{SyntaxTokenWithParent, TokenKind, token::TokenKindExt};

/// Precedence for navigation features (goto definition/declaration,
/// references, document highlight): name-like tokens and pair tokens
/// (``begin``/``end``, ``module``/``endmodule``, ...) win over punctuation.
pub(crate) fn navigation_precedence(kind: TokenKind) -> usize {
    match kind {
        _ if kind.name_like() => 4,
        _ if kind.is_pair_token() => 4,
        _ => 1,
    }
}

/// Precedence for hover: name-like tokens win, literals rank above
/// punctuation so hovering a literal shows its value.
pub(crate) fn hover_precedence(kind: TokenKind) -> usize {
    match kind {
        _ if kind.name_like() => 4,
        _ if kind.is_literal() => 3,
        _ => 1,
    }
}

/// Precedence for the semantic index build: only name-like tokens are
/// indexed, so the function is a boolean predicate.
pub(crate) fn name_precedence(kind: TokenKind) -> usize {
    usize::from(kind.name_like())
}

/// The beg and end tokens of a pair (``begin``/``end``, ``module``/
/// ``endmodule``, ...), when `tp` is one side of a pair.
pub(crate) fn ctrl_flow_pair(
    tp: SyntaxTokenWithParent<'_>,
) -> Option<(SyntaxTokenWithParent<'_>, SyntaxTokenWithParent<'_>)> {
    let pair = syntax::pair_token(tp)?;
    let (beg, end) = pair.either(|beg| (beg, tp), |end| (tp, end));
    Some((beg, end))
}
