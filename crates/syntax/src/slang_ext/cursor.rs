use slang::SyntaxCursor;
use utils::line_index::TextSize;

pub trait SyntaxCursorExt {
    fn goto_first_tok_after(&mut self, offset: TextSize) -> bool;

    fn goto_first_tok_after_or_last(&mut self, offset: TextSize) -> bool;

    fn goto_last_tok_before(&mut self, offset: TextSize) -> bool;
}

impl SyntaxCursorExt for SyntaxCursor<'_> {
    fn goto_first_tok_after(&mut self, offset: TextSize) -> bool {
        let offset: usize = offset.into();
        let Some(end) = self.to_elem().range().map(|range| range.end()) else {
            return false;
        };
        if end <= offset {
            return false;
        }

        while self.to_node().is_some() {
            let success = self.goto_first_child_after_pos(offset);
            if !success {
                return false;
            }
        }
        debug_assert!(self.to_token().is_some());
        true
    }

    fn goto_first_tok_after_or_last(&mut self, offset: TextSize) -> bool {
        if !self.goto_first_tok_after(offset) {
            if self.to_elem().range().is_some_and(|range| range.end() == usize::from(offset)) {
                while self.to_node().is_some() {
                    let success = self.goto_last_child();
                    if !success {
                        return false;
                    }
                }
            } else {
                return false;
            }
        }
        true
    }

    fn goto_last_tok_before(&mut self, offset: TextSize) -> bool {
        let offset: usize = offset.into();
        let Some(start) = self.to_elem().range().map(|range| range.start()) else {
            return false;
        };
        if start >= offset {
            return false;
        }

        while self.to_node().is_some() {
            let success = self.goto_last_child_before_pos(offset);
            if !success {
                return false;
            }
        }
        debug_assert!(self.to_token().is_some());
        true
    }
}
