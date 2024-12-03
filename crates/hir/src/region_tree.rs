use la_arena::{Arena, Idx};
use smol_str::SmolStr;
use syntax::{
    SyntaxNode, SyntaxNodeExt, SyntaxToken, SyntaxTrivia, WalkEvent, has_text_range::HasTextRange,
    token::SyntaxTokenExt, trivia::TriviaExt,
};
use utils::text_edit::{TextRange, TextSize};

// items, decls, stmts
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum RegionKind {
    Region { name: Option<SmolStr>, begin_range: TextRange },
    PseudoRegion,
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct RegionTree {
    pub roots: Vec<Idx<RegionNode>>,
    pub nodes: Arena<RegionNode>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct RegionNode {
    pub range: TextRange,
    pub kind: RegionKind,
    pub children: Vec<Idx<RegionNode>>,
    pub parent: Option<Idx<RegionNode>>,
}

impl RegionTree {
    pub(crate) fn add_node(
        &mut self,
        range: TextRange,
        kind: RegionKind,
        parent: Option<Idx<RegionNode>>,
    ) -> Idx<RegionNode> {
        let idx = self.nodes.alloc(RegionNode { range, kind, children: Vec::new(), parent });
        if let Some(parent) = parent {
            self.nodes[parent].children.push(idx);
        } else {
            self.roots.push(idx);
        }
        idx
    }

    pub fn walk(&self) -> RegionTreeIterator<'_> {
        RegionTreeIterator::new(self)
    }

    pub fn find(&self, offset: TextSize) -> Option<Idx<RegionNode>> {
        let mut idx = Self::find_in_node(&self.nodes, &self.roots, offset)?;

        loop {
            let node = &self.nodes[idx];
            if node.children.is_empty() {
                return Some(idx);
            }
            if let Some(new_idx) = Self::find_in_node(&self.nodes, &node.children, offset) {
                idx = new_idx;
            } else {
                return Some(idx);
            }
        }
    }

    fn find_in_node(
        nodes: &Arena<RegionNode>,
        children: &[Idx<RegionNode>],
        offset: TextSize,
    ) -> Option<Idx<RegionNode>> {
        let idx = children
            .binary_search_by(|&idx| {
                let node = &nodes[idx];
                if node.range.contains(offset) {
                    std::cmp::Ordering::Equal
                } else if node.range.start() > offset {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Less
                }
            })
            .ok()?;
        Some(children[idx])
    }
}

impl RegionNode {
    const REGION_DEFAULT_NAME: &SmolStr = &SmolStr::new_static("<region>");

    pub fn name(&self) -> &SmolStr {
        match &self.kind {
            RegionKind::Region { name, .. } => name.as_ref().unwrap_or(Self::REGION_DEFAULT_NAME),
            _ => Self::REGION_DEFAULT_NAME,
        }
    }

    pub fn focus_range(&self) -> TextRange {
        match &self.kind {
            RegionKind::Region { begin_range, .. } => *begin_range,
            RegionKind::PseudoRegion => self.range,
        }
    }
}

#[derive(Debug)]
pub(crate) struct RegionTreeBuilder {
    tree: RegionTree,
    stack: Vec<Idx<RegionNode>>,
}

impl RegionTreeBuilder {
    pub(crate) fn new() -> Self {
        Self { tree: RegionTree::default(), stack: Vec::new() }
    }

    fn open_region(&mut self, start: usize, kind: RegionKind) {
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

    pub(crate) fn stage(&mut self, end_tok: Option<SyntaxToken>) {
        let end = end_tok.unwrap().text_range().unwrap().end();
        while let Some(last) = self.stack.last() {
            let node = &mut self.tree.nodes[*last];
            let start = node.range.start();
            node.range = TextRange::new(start, end);
            self.stack.pop();
        }
        self.handle_tok(end_tok);
    }

    pub(crate) fn finish(&mut self) -> RegionTree {
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
            self.open_region(range.start().into(), RegionKind::Region { name, begin_range: range });
        } else if trivia.is_region_end() {
            self.finish_region(range.end().into());
        } else {
            // TODO: handle doc comments
        }
    }
}

pub struct RegionTreeIterator<'a> {
    tree: &'a RegionTree,
    stack: Vec<(Idx<RegionNode>, bool)>, // (node_idx, visited)
}

impl<'a> RegionTreeIterator<'a> {
    fn new(tree: &'a RegionTree) -> Self {
        let stack = tree.roots.iter().rev().map(|&idx| (idx, false)).collect();

        Self { tree, stack }
    }
}

impl<'a> Iterator for RegionTreeIterator<'a> {
    type Item = WalkEvent<&'a RegionNode>;

    fn next(&mut self) -> Option<Self::Item> {
        let &mut (node_idx, ref mut visited) = self.stack.last_mut()?;

        if !*visited {
            *visited = true;
            let children = self.tree.nodes[node_idx].children.iter().rev().map(|&idx| (idx, false));
            self.stack.extend(children);
            Some(WalkEvent::Enter(&self.tree.nodes[node_idx]))
        } else {
            self.stack.pop();
            Some(WalkEvent::Leave(&self.tree.nodes[node_idx]))
        }
    }
}

#[derive(Debug)]
pub struct RegionParent<'a> {
    tree: &'a RegionTree,
    node: Option<Idx<RegionNode>>,
}

impl<'a> RegionParent<'a> {
    pub fn start_from(tree: &'a RegionTree, node: Idx<RegionNode>) -> Self {
        Self { tree, node: Some(node) }
    }
}

impl<'a> Iterator for RegionParent<'a> {
    type Item = &'a RegionNode;

    fn next(&mut self) -> Option<Self::Item> {
        let node = &self.tree.nodes[self.node?];
        self.node = node.parent;
        Some(node)
    }
}
