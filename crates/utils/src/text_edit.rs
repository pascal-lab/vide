use std::cmp::max;

use itertools::Itertools;

pub use crate::line_index::{TextRange, TextSize};

// A single atomic change to text: a insertion, a deletion or a replacement.
// Must not overlap with other `InDel`s
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextEditItem {
    pub ins: String,
    /// Refers to offsets in the original text
    pub del: TextRange,
}

impl TextEditItem {
    pub fn insert(offset: TextSize, text: String) -> TextEditItem {
        TextEditItem::replace(TextRange::empty(offset), text)
    }

    pub fn delete(range: TextRange) -> TextEditItem {
        TextEditItem::replace(range, String::new())
    }

    pub fn replace(range: TextRange, replace_with: String) -> TextEditItem {
        TextEditItem { del: range, ins: replace_with }
    }

    pub fn apply_on(&self, text: &mut String) {
        let start: usize = self.del.start().into();
        let end: usize = self.del.end().into();
        text.replace_range(start..end, &self.ins);
    }
}

#[derive(Default, Debug, Clone)]
pub struct TextEdit {
    /// Invariant: disjoint and sorted by `delete`.
    changes: Vec<TextEditItem>,
}

impl TextEdit {
    pub fn builder() -> TextEditBuilder {
        TextEditBuilder::default()
    }

    pub fn insert(offset: TextSize, text: String) -> TextEdit {
        let mut builder = TextEdit::builder();
        builder.insert(offset, text);
        builder.finish()
    }

    pub fn delete(range: TextRange) -> TextEdit {
        let mut builder = TextEdit::builder();
        builder.delete(range);
        builder.finish()
    }

    pub fn replace(range: TextRange, replace_with: String) -> TextEdit {
        let mut builder = TextEdit::builder();
        builder.replace(range, replace_with);
        builder.finish()
    }

    pub fn len(&self) -> usize {
        self.changes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, TextEditItem> {
        self.into_iter()
    }

    pub fn apply(&self, text: &mut String) {
        match self.len() {
            0 => return,
            1 => {
                if let Some(change) = self.changes.first() {
                    change.apply_on(text);
                }
                return;
            }
            _ => (),
        }

        let text_size = TextSize::of(&*text);
        let mut change_len = text_size;
        let mut max_change_len = text_size;
        for change in &self.changes {
            change_len += TextSize::of(&change.ins);
            change_len -= change.del.len();
            max_change_len = max(max_change_len, change_len);
        }

        if let Some(additional) = max_change_len.checked_sub(text_size) {
            text.reserve(additional.into());
        }

        for change in self.changes.iter().rev() {
            change.apply_on(text);
        }

        assert_eq!(TextSize::of(&*text), change_len);
    }

    pub fn union(&mut self, other: TextEdit) -> Result<(), TextEdit> {
        let iter_merge = self.iter().merge_by(other.iter(), |l, r| l.del.start() <= r.del.start());

        if !check_all_disjoint(iter_merge.clone()) {
            return Err(other);
        }

        // Only dedup deletions and replacements, keep all insertions (delete.is_empty)
        self.changes = iter_merge.dedup_by(|a, b| a == b && !a.del.is_empty()).cloned().collect();

        Ok(())
    }

    pub fn apply_to_offset(&self, offset: TextSize) -> Option<TextSize> {
        let mut res = offset;
        for change in &self.changes {
            if change.del.start() >= offset {
                break;
            }
            if offset < change.del.end() {
                return None;
            }
            res += TextSize::of(&change.ins);
            res -= change.del.len();
        }
        Some(res)
    }
}

impl IntoIterator for TextEdit {
    type IntoIter = std::vec::IntoIter<TextEditItem>;
    type Item = TextEditItem;

    fn into_iter(self) -> Self::IntoIter {
        self.changes.into_iter()
    }
}

impl<'a> IntoIterator for &'a TextEdit {
    type IntoIter = std::slice::Iter<'a, TextEditItem>;
    type Item = &'a TextEditItem;

    fn into_iter(self) -> Self::IntoIter {
        self.changes.iter()
    }
}

impl std::iter::FromIterator<TextEditItem> for TextEdit {
    fn from_iter<I: IntoIterator<Item = TextEditItem>>(iter: I) -> Self {
        let mut changes = iter.into_iter().collect_vec();
        assert!(sort_and_check_disjoint(&mut changes));
        changes = coalsece_changes(changes);
        TextEdit { changes }
    }
}

#[derive(Debug, Default, Clone)]
pub struct TextEditBuilder {
    changes: Vec<TextEditItem>,
}

impl TextEditBuilder {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn replace(&mut self, range: TextRange, with: String) {
        self.change(TextEditItem::replace(range, with));
    }

    pub fn delete(&mut self, range: TextRange) {
        self.change(TextEditItem::delete(range));
    }

    pub fn insert(&mut self, offset: TextSize, text: String) {
        self.change(TextEditItem::insert(offset, text));
    }

    pub fn finish(self) -> TextEdit {
        let mut changes = self.changes;
        assert!(sort_and_check_disjoint(&mut changes));
        changes = coalsece_changes(changes);
        TextEdit { changes }
    }

    pub fn invalidates_offset(&self, offset: TextSize) -> bool {
        self.changes.iter().any(|change| change.del.contains_inclusive(offset))
    }

    fn change(&mut self, change: TextEditItem) {
        self.changes.push(change);
    }
}

fn sort_and_check_disjoint(changes: &mut [TextEditItem]) -> bool {
    changes.sort_by_key(|change| (change.del.start(), change.del.end()));
    check_all_disjoint(changes.iter())
}

fn check_all_disjoint<'a, I>(changes: I) -> bool
where
    I: std::iter::Iterator<Item = &'a TextEditItem> + Clone,
{
    changes.clone().zip(changes.skip(1)).all(|(l, r)| l.del.end() <= r.del.start() || l == r)
}

fn coalsece_changes(changes: Vec<TextEditItem>) -> Vec<TextEditItem> {
    changes
        .into_iter()
        .coalesce(|mut a, b| {
            if a.del.end() == b.del.start() {
                a.ins.push_str(&b.ins);
                a.del = TextRange::new(a.del.start(), b.del.end());
                Ok(a)
            } else {
                Err((a, b))
            }
        })
        .collect_vec()
}
