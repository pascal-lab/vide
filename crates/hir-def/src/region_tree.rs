use base_db::salsa;
use la_arena::{Arena, Idx};
use smol_str::{SmolStr, ToSmolStr};
use syntax::{
    ChildrenIter, SyntaxElement, SyntaxNode, SyntaxNodeExt, SyntaxToken, SyntaxTrivia, WalkEvent,
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
    match_ast,
    token::SyntaxTokenExt,
    trivia::{TriviaExt, TriviaKindExt},
};
use triomphe::Arc;
use utils::text_edit::{TextRange, TextSize};

use crate::{
    db::HirDefDb,
    owner::{OwnerId, OwnerKind},
};

// items, decls, stmts
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum RegionKind {
    Region { name: Option<SmolStr>, begin_range: TextRange },
    PseudoRegion { description: Option<SmolStr> },
}

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct RegionTree {
    roots: Vec<Idx<RegionNode>>,
    nodes: Arena<RegionNode>,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
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

    pub fn root_count(&self) -> usize {
        self.roots.len()
    }

    pub fn nodes(&self) -> impl Iterator<Item = &RegionNode> {
        self.nodes.values()
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
        let candidate_count = children.partition_point(|&idx| nodes[idx].range.start() <= offset);
        children[..candidate_count]
            .iter()
            .rev()
            .find(|&&idx| nodes[idx].range.contains(offset))
            .copied()
    }

    fn normalize(&mut self) {
        let roots = std::mem::take(&mut self.roots);
        self.roots = Self::normalize_children(&mut self.nodes, roots);
    }

    fn normalize_children(
        nodes: &mut Arena<RegionNode>,
        mut children: Vec<Idx<RegionNode>>,
    ) -> Vec<Idx<RegionNode>> {
        children.sort_by(|left, right| {
            nodes[*left]
                .range
                .start()
                .cmp(&nodes[*right].range.start())
                .then_with(|| nodes[*right].range.end().cmp(&nodes[*left].range.end()))
        });

        for idx in children.iter().copied() {
            let nested = std::mem::take(&mut nodes[idx].children);
            nodes[idx].children = Self::normalize_children(nodes, nested);
        }

        children
    }
}

impl RegionNode {
    const REGION_DEFAULT_NAME: &SmolStr = &SmolStr::new_static("<region>");

    pub fn name(&self) -> &SmolStr {
        let name = match &self.kind {
            RegionKind::Region { name, .. } => name.as_ref(),
            RegionKind::PseudoRegion { description } => description.as_ref(),
        };
        name.unwrap_or(Self::REGION_DEFAULT_NAME)
    }

    pub fn focus_range(&self) -> TextRange {
        match &self.kind {
            RegionKind::Region { begin_range, .. } => *begin_range,
            RegionKind::PseudoRegion { .. } => self.range,
        }
    }
}

#[derive(Debug)]
pub(crate) struct RegionTreeBuilder {
    tree: RegionTree,
    stack: Vec<Idx<RegionNode>>,
    pseudo_region: Option<(usize, TextRange, SmolStr)>,
}

impl RegionTreeBuilder {
    pub(crate) fn new() -> Self {
        Self { tree: RegionTree::default(), stack: Vec::new(), pseudo_region: None }
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

    pub(crate) fn stage<'a>(&mut self, end_tok: Option<SyntaxToken<'a>>, context: SyntaxNode<'a>) {
        // Scan the end token's trivia first: an `endregion` marker attached to
        // the closing token must close its region, otherwise every open region
        // would be force-closed below and the marker would be lost.
        self.handle_tok(end_tok, context);
        if let Some(end) =
            end_tok.as_ref().and_then(|tok| tok.text_range_in(context)).map(|r| r.end())
        {
            while let Some(last) = self.stack.last() {
                let node = &mut self.tree.nodes[*last];
                let start = node.range.start();
                node.range = TextRange::new(start, end);
                self.stack.pop();
            }
        }
    }

    pub(crate) fn finish(&mut self) -> RegionTree {
        self.tree.normalize();
        self.tree.nodes.shrink_to_fit();
        self.tree.roots.shrink_to_fit();
        std::mem::take(&mut self.tree)
    }

    #[inline]
    pub(crate) fn handle_node(&mut self, node: SyntaxNode) {
        self.handle_pseudo_region(node, node.trivias());
        self.handle_trivia(node.trivias_with_range());
    }

    #[inline]
    fn handle_tok(&mut self, token: Option<SyntaxToken>, context: SyntaxNode) {
        let Some(token) = token else {
            return;
        };

        self.finish_pseudo_region();
        self.handle_trivia(token.trivias_with_range_in_root(context.find_root()));
    }

    fn handle_pseudo_region<'a>(
        &mut self,
        node: SyntaxNode<'a>,
        trivias: impl ChildrenIter<SyntaxTrivia<'a>>,
    ) {
        match_ast! { node,
            ast::DataDeclaration
            | ast::NetDeclaration
            | ast::ParameterDeclaration
            | ast::ImplicitAnsiPort
            | ast::PortDeclaration => {},
            _ => {
                self.finish_pseudo_region();
                return;
            },
        };

        let trivias = trivias.rev().filter(|t| !t.kind().is_whitespace());
        if let Some((cnt, range, _)) = self.pseudo_region.as_mut() {
            let mut trivias = trivias.clone();

            let first_eol = trivias.next();
            let second_eol = trivias.find(|t| t.kind().is_eol());
            if first_eol.is_none_or(|t| t.kind().is_eol()) && second_eol.is_none() {
                let Some(node_range) = node.text_range() else {
                    self.finish_pseudo_region();
                    return;
                };
                *cnt += 1;
                *range = range.cover(node_range);
                return;
            } else {
                self.finish_pseudo_region();
            }
        }

        // set self.pseudo_region
        let mut trivias = trivias.peekable();
        let mut last_comment = None;

        trivias.next_if(|t| t.kind().is_eol());
        loop {
            if let Some(comment) = trivias.next_if(|t| t.kind().is_comment())
                && comment.is_region_begin().is_none()
                && !comment.is_region_end()
            {
                last_comment = Some(comment);
            } else if trivias.next_if(|t| t.kind().is_eol()).is_some() {
                break;
            } else {
                return;
            }
        }

        if let Some(description) =
            last_comment.and_then(|comment| comment.as_comment().map(|text| text.to_smolstr()))
            && let Some(range) = node.text_range()
        {
            self.pseudo_region = Some((1, range, description));
        }
    }

    fn finish_pseudo_region(&mut self) {
        if let Some((cnt, range, description)) = self.pseudo_region.take()
            && cnt > 1
        {
            let kind = RegionKind::PseudoRegion { description: Some(description) };
            self.open_region(range.start().into(), kind);
            self.finish_region(range.end().into());
        };
    }

    #[inline]
    fn handle_trivia<'a>(&'a mut self, trivias: impl ChildrenIter<(TextRange, SyntaxTrivia<'a>)>) {
        for (range, trivia) in trivias {
            if let Some(name) = trivia.is_region_begin() {
                let region = RegionKind::Region { name, begin_range: range };
                self.open_region(range.start().into(), region);
            } else if trivia.is_region_end() {
                self.finish_region(range.end().into());
            }
        }
    }
}

/// Current source regions for one canonical owner. Region ranges and comments
/// are revision-local source data, so they are deliberately not stored in the
/// position-free owner HIR.
#[salsa::tracked(lru = 512, returns(clone))]
pub(crate) fn owner_region_tree(db: &dyn HirDefDb, owner: OwnerId) -> Arc<RegionTree> {
    let owner = match owner.kind(db) {
        OwnerKind::Checker | OwnerKind::Covergroup | OwnerKind::ClockingBlock => {
            owner.parent(db).expect("scope-only owner must have a body owner parent")
        }
        _ => owner,
    };
    let file_id = owner.file(db);
    let tree = db.parse(file_id);
    let Some(root) = db.ast_id_map(file_id).node(owner.ast_id(db), &tree) else {
        return Arc::new(RegionTree::default());
    };

    let mut builder = RegionTreeBuilder::new();
    collect_owner_regions(&mut builder, root, owner.kind(db));
    builder.stage(last_token(root), root);
    Arc::new(builder.finish())
}

pub(crate) fn set_region_tree_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    owner_region_tree::set_lru_capacity(db, capacity);
}

fn collect_owner_regions(builder: &mut RegionTreeBuilder, root: SyntaxNode<'_>, kind: OwnerKind) {
    match kind {
        OwnerKind::File => {
            if let Some(file) = ast::CompilationUnit::cast(root) {
                for member in file.members().children() {
                    builder.handle_node(member.syntax());
                }
            } else if let Some(file) = ast::LibraryMap::cast(root) {
                for member in file.members().children() {
                    builder.handle_node(member.syntax());
                }
            }
        }
        OwnerKind::Module => {
            let Some(module) = ast::ModuleDeclaration::cast(root) else {
                return;
            };
            let header = module.header();
            if let Some(parameters) = header.parameters() {
                for declaration in parameters.declarations().children() {
                    builder.handle_node(declaration.syntax());
                }
            }
            match header.ports() {
                Some(ast::PortList::AnsiPortList(ports)) => {
                    for port in ports.ports().children() {
                        builder.handle_node(port.syntax());
                    }
                }
                Some(ast::PortList::NonAnsiPortList(ports)) => {
                    for port in ports.ports().children() {
                        builder.handle_node(port.syntax());
                    }
                }
                Some(ast::PortList::WildcardPortList(_)) | None => {}
            }
            for member in module.members().children() {
                builder.handle_node(member.syntax());
            }
        }
        OwnerKind::GenerateBlock => {
            if let Some(block) = ast::GenerateBlock::cast(root) {
                for member in block.members().children() {
                    builder.handle_node(member.syntax());
                }
            } else if let Some(loop_generate) = ast::LoopGenerate::cast(root)
                && let Some(block) = loop_generate.block().as_generate_block()
            {
                for member in block.members().children() {
                    builder.handle_node(member.syntax());
                }
            }
        }
        OwnerKind::Subroutine => {
            if let Some(subroutine) = ast::FunctionDeclaration::cast(root) {
                for item in subroutine.items().children() {
                    builder.handle_node(item.syntax());
                }
            }
        }
        OwnerKind::Block => {
            if let Some(block) = ast::BlockStatement::cast(root) {
                for item in block.items().children() {
                    builder.handle_node(item.syntax());
                }
            }
        }
        OwnerKind::ProceduralBlock => {
            if let Some(proc) = ast::ProceduralBlock::cast(root) {
                builder.handle_node(proc.statement().syntax());
            }
        }
        OwnerKind::Checker | OwnerKind::Covergroup | OwnerKind::ClockingBlock => {
            for child in root.children() {
                if let SyntaxElement::Node(node) = child {
                    builder.handle_node(node);
                }
            }
        }
    }
}

fn last_token(node: SyntaxNode<'_>) -> Option<SyntaxToken<'_>> {
    node.elem_preorder()
        .filter_map(|event| match event {
            WalkEvent::Enter(SyntaxElement::Token(token)) => Some(token.tok),
            _ => None,
        })
        .last()
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

#[cfg(test)]
mod tests {
    use utils::text_edit::{TextRange, TextSize};

    use super::{RegionKind, RegionTree};

    fn range(start: u32, end: u32) -> TextRange {
        TextRange::new(TextSize::new(start), TextSize::new(end))
    }

    #[test]
    fn find_is_independent_of_builder_insertion_order() {
        let mut tree = RegionTree::default();
        let late =
            tree.add_node(range(10, 20), RegionKind::PseudoRegion { description: None }, None);
        let early =
            tree.add_node(range(0, 5), RegionKind::PseudoRegion { description: None }, None);

        tree.normalize();

        assert_eq!(tree.find(TextSize::new(2)), Some(early));
        assert_eq!(tree.find(TextSize::new(12)), Some(late));
    }

    #[test]
    fn find_descends_into_sorted_children() {
        let mut tree = RegionTree::default();
        let parent =
            tree.add_node(range(0, 30), RegionKind::PseudoRegion { description: None }, None);
        let late = tree.add_node(
            range(20, 25),
            RegionKind::PseudoRegion { description: None },
            Some(parent),
        );
        let early = tree.add_node(
            range(5, 10),
            RegionKind::PseudoRegion { description: None },
            Some(parent),
        );

        tree.normalize();

        assert_eq!(tree.find(TextSize::new(7)), Some(early));
        assert_eq!(tree.find(TextSize::new(22)), Some(late));
    }
}
