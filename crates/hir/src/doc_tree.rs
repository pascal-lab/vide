use la_arena::{Arena, Idx};
use smol_str::SmolStr;
use syntax::{
    SyntaxNode, SyntaxNodeExt, SyntaxToken, SyntaxTrivia, token::SyntaxTokenExt, trivia::TriviaExt,
};
use utils::text_edit::{TextRange, TextSize};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum DocKind {
    Region(SmolStr),
    PseudoRegion,
}

// 1. region .. endregion
// 2. comment (optional) ...more than one line declarators
#[derive(Default, Debug, PartialEq, Eq)]
pub struct DocTree {
    pub roots: Vec<Idx<DocNode>>,
    pub nodes: Arena<DocNode>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct DocNode {
    pub range: TextRange,
    pub kind: DocKind,
    pub children: Vec<Idx<DocNode>>,
}

impl DocTree {
    pub(crate) fn add_node(
        &mut self,
        range: TextRange,
        kind: DocKind,
        parent: Option<Idx<DocNode>>,
    ) -> Idx<DocNode> {
        let idx = self.nodes.alloc(DocNode { range, kind, children: Vec::new() });
        if let Some(parent) = parent {
            self.nodes[parent].children.push(idx);
        } else {
            self.roots.push(idx);
        }
        idx
    }
}

#[derive(Debug)]
pub(crate) struct DocTreeBuilder {
    tree: DocTree,
    stack: Vec<Idx<DocNode>>,
}

impl DocTreeBuilder {
    pub(crate) fn new() -> Self {
        Self { tree: DocTree::default(), stack: Vec::new() }
    }

    fn open_region(&mut self, start: usize, kind: DocKind) {
        let parent = self.stack.last().copied();
        let range = TextRange::empty(TextSize::new(start as u32));
        let node = self.tree.add_node(range, kind, parent);
        self.stack.push(node);
    }

    fn finish_region(&mut self, end: usize) {
        let Some(last) = self.stack.last() else {
            // TODO: diagnostics for empty stack
            return;
        };
        let node = &mut self.tree.nodes[*last];
        let start = node.range.start();
        let end = TextSize::new(end as u32);
        node.range = TextRange::new(start, end);
        self.stack.pop();
    }

    fn add_node(&mut self, range: TextRange, kind: DocKind) {
        self.tree.add_node(range, kind, self.stack.last().copied());
    }

    // TODO: diagnostics for !is_empty
    pub(crate) fn check_empty(&self) {}

    pub(crate) fn finish(&mut self) -> DocTree {
        self.check_empty();
        self.tree.nodes.shrink_to_fit();
        self.tree.roots.shrink_to_fit();
        std::mem::take(&mut self.tree)
    }

    #[inline]
    pub(crate) fn handle_node(&mut self, node: SyntaxNode) {
        node.trivias_with_range().for_each(|(range, trivia)| self.handle_trivia(range, trivia));
    }

    #[inline]
    pub(crate) fn handle_tok(&mut self, token: Option<SyntaxToken>) {
        let Some(token) = token else {
            return;
        };
        token.trivias_with_range().for_each(|(range, trivia)| self.handle_trivia(range, trivia));
    }

    #[inline]
    fn handle_trivia(&mut self, range: TextRange, trivia: SyntaxTrivia) {
        if let Some(name) = trivia.is_region_begin() {
            self.open_region(range.start().into(), DocKind::Region(name));
        } else if trivia.is_region_end() {
            self.finish_region(range.end().into());
        } else {
            // TODO: handle doc comments
        }
    }
}
