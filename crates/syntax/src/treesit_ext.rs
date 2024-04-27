use crate::{syntax_kind::SyntaxKindId, SyntaxNode};
use line_index::TextSize;
use utils::text_edit::to_text_range;

#[derive(Clone, Debug)]
pub enum TokenAtOffset<'a> {
    None,
    Single(SyntaxNode<'a>),
    Between(SyntaxNode<'a>, SyntaxNode<'a>),
}

pub fn pick_best_token(
    tokens: TokenAtOffset,
    f: impl Fn(SyntaxKindId) -> usize,
) -> Option<SyntaxNode> {
    match tokens {
        TokenAtOffset::None => None,
        TokenAtOffset::Single(n) => Some(n),
        TokenAtOffset::Between(a, b) => {
            if f(a.kind_id()) > f(b.kind_id()) {
                Some(a)
            } else {
                Some(b)
            }
        }
    }
}

pub fn token_at_offset<'a>(root_node: &SyntaxNode<'a>, offset: TextSize) -> TokenAtOffset<'a> {
    let range = to_text_range(root_node.range());
    assert!(range.contains(offset));
    if range.is_empty() {
        return TokenAtOffset::None;
    }

    let left = token_before_or_after_offset(root_node, offset);
    let left_range = to_text_range(left.range());
    if !left_range.contains_inclusive(offset) {
        return TokenAtOffset::None;
    } else if left_range.contains(offset) {
        return TokenAtOffset::Single(left);
    }

    assert!(offset == left_range.end());
    let right = left
        .next_sibling()
        .map(|node| leftmost_leaf(node))
        .unwrap_or_else(|| token_before_or_after_offset(root_node, offset + TextSize::from(1)));
    let right_range = to_text_range(right.range());
    if right_range.start() == left_range.end() {
        TokenAtOffset::Between(left, right)
    } else {
        TokenAtOffset::Single(left)
    }
}

pub fn token_before_or_after_offset<'a>(
    root_node: &SyntaxNode<'a>,
    offset: impl Into<usize>,
) -> SyntaxNode<'a> {
    let offset = offset.into();
    let mut cursor = root_node.walk();
    loop {
        if cursor.goto_first_child_for_byte(offset).is_none() {
            break;
        }
    }
    cursor.node()
}

pub fn leftmost_leaf(node: SyntaxNode) -> SyntaxNode {
    let mut cursor = node.walk();
    loop {
        if !cursor.goto_first_child() {
            break;
        }
    }
    cursor.node()
}

pub fn find_root(node: SyntaxNode) -> SyntaxNode {
    let mut cursor = node.walk();
    loop {
        if !cursor.goto_parent() {
            break;
        }
    }
    cursor.node()
}
