use std::iter;

use utils::text_edit::TextRange;

use super::SemaToken;

#[derive(Debug)]
pub(super) struct SemaTokenCollectorTree {
    root: Node,
}

#[derive(Debug)]
struct Node {
    tok: SemaToken,
    nested: Vec<Node>,
}

impl SemaTokenCollectorTree {
    pub(super) fn new(tok: SemaToken) -> SemaTokenCollectorTree {
        SemaTokenCollectorTree { root: Node::new(tok) }
    }

    pub(super) fn add(&mut self, tok: SemaToken) {
        self.root.add(tok);
    }

    pub(super) fn finish(&self) -> Vec<SemaToken> {
        let mut res = Vec::new();
        self.root.flatten(&mut res);
        res
    }
}

impl Node {
    fn new(tok: SemaToken) -> Node {
        Node { tok, nested: Vec::new() }
    }

    fn add(&mut self, tok: SemaToken) {
        if !self.tok.range.contains_range(tok.range) {
            return;
        }

        if let Some(last) = self.nested.last_mut() {
            if last.tok.range.contains_range(tok.range) {
                last.add(tok);
                return;
            }
            if last.tok.range.end() <= tok.range.start() {
                return self.nested.push(Node::new(tok));
            }
        }

        let overlapping = {
            let start = self.nested.partition_point(|it| it.tok.range.end() <= tok.range.start());
            let Some(rest) = self.nested.get(start..) else {
                return;
            };
            let len = rest.partition_point(|it| it.tok.range.intersect(tok.range).is_some());
            start..start + len
        };

        if overlapping.len() == 1
            && let Some(node) = self.nested.get_mut(overlapping.start)
            && node.tok.range.contains_range(tok.range)
        {
            return node.add(tok);
        }

        let nested = self.nested.splice(overlapping.clone(), iter::once(Node::new(tok))).collect();
        if let Some(node) = self.nested.get_mut(overlapping.start) {
            node.nested = nested;
        }
    }

    fn flatten(&self, acc: &mut Vec<SemaToken>) {
        let SemaToken { range, tag, mods } = self.tok;
        let mut start = range.start();
        let mut nested = self.nested.iter();
        loop {
            let next = nested.next();
            let end = next.map_or(range.end(), |it| it.tok.range.start());
            if start < end {
                acc.push(SemaToken { range: TextRange::new(start, end), tag, mods });
            }
            start = match next {
                Some(child) => {
                    child.flatten(acc);
                    child.tok.range.end()
                }
                None => break,
            }
        }
    }
}
