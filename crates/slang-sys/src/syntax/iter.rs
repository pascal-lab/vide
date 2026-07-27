use super::{element::SyntaxElement, syntax_node::SyntaxNode};

/// Base iterator over a node's children that yields each child with its raw
/// child index.
#[derive(Clone, Debug)]
pub struct SyntaxIdxChildren<'a> {
    pub(crate) parent: SyntaxNode<'a>,
    pub(crate) start_idx: usize,
    pub(crate) end_idx: usize,
}

/// Iterator over a node's child elements.
#[derive(Clone, Debug)]
pub struct SyntaxChildren<'a>(pub(crate) SyntaxIdxChildren<'a>);

/// Trait alias for iterators over syntax children.
pub trait ChildrenIter<It>: DoubleEndedIterator<Item = It> + ExactSizeIterator + Clone {}

impl<T, It> ChildrenIter<It> for T where
    T: DoubleEndedIterator<Item = It> + ExactSizeIterator + Clone
{
}

impl<'a> Iterator for SyntaxIdxChildren<'a> {
    type Item = (usize, SyntaxElement<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        while self.start_idx < self.end_idx {
            let index = self.start_idx;
            self.start_idx += 1;
            if let Some(child) = self.parent.child(index) {
                return Some((index, child));
            }
        }
        None
    }
}

impl<'a> DoubleEndedIterator for SyntaxIdxChildren<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        while self.start_idx < self.end_idx {
            self.end_idx -= 1;
            if let Some(child) = self.parent.child(self.end_idx) {
                return Some((self.end_idx, child));
            }
        }
        None
    }
}

impl ExactSizeIterator for SyntaxIdxChildren<'_> {
    fn len(&self) -> usize {
        self.end_idx - self.start_idx
    }
}

impl<'a> Iterator for SyntaxChildren<'a> {
    type Item = SyntaxElement<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, elem)| elem)
    }
}

impl<'a> DoubleEndedIterator for SyntaxChildren<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(_, elem)| elem)
    }
}

impl ExactSizeIterator for SyntaxChildren<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

/// Iterator over a syntax node and its parents.
pub struct SyntaxAncestors<'a> {
    node: Option<SyntaxNode<'a>>,
}

impl<'a> SyntaxAncestors<'a> {
    pub fn start_from(node: SyntaxNode<'a>) -> Self {
        Self { node: Some(node) }
    }
}

impl<'a> Iterator for SyntaxAncestors<'a> {
    type Item = SyntaxNode<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.node.take()?;
        self.node = node.parent();
        Some(node)
    }
}
