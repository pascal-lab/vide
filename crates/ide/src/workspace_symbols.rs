use std::{cmp::Ordering, collections::BinaryHeap};

use fst::{IntoStreamer, Streamer};
use hir::{
    base_db::{source_db::SourceRootDb, source_root::SourceRootId},
    db::HirDb,
};
use rayon::prelude::*;
use triomphe::Arc;
use utils::line_index::TextRange;
use vfs::FileId;

use crate::{
    SymbolKind,
    db::{
        root_db::RootDb,
        workspace_symbol_index_db::{WorkspaceSymbolIndexDb, source_root_symbol_index_for_root},
    },
    document_symbols::{self, DocumentSymbol},
};

const WORKSPACE_SYMBOL_LIMIT: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSymbol {
    pub file_id: FileId,
    pub name: String,
    pub focus_range: TextRange,
    pub full_range: TextRange,
    pub kind: SymbolKind,
    pub container_name: Option<String>,
}

pub(crate) fn workspace_symbols(
    db: &RootDb,
    query: &str,
    file_ids: Vec<FileId>,
) -> Vec<WorkspaceSymbol> {
    let query = Query::new(query);
    let root_ids = unique_source_root_ids(db, file_ids);
    let indexes = root_ids
        .into_iter()
        .map(|source_root_id| source_root_symbol_index_for_root(db, source_root_id))
        .collect::<Vec<_>>();
    let mut symbols = BinaryHeap::new();
    for index in &indexes {
        query.search(index, |symbol| {
            let symbol = RankedSymbol(symbol);
            if symbols.len() < WORKSPACE_SYMBOL_LIMIT {
                symbols.push(symbol);
            } else if symbols.peek().is_some_and(|worst| symbol < *worst) {
                symbols.pop();
                symbols.push(symbol);
            }
        });
    }
    let mut symbols = symbols.into_iter().map(|symbol| symbol.0).collect::<Vec<_>>();
    symbols.sort_unstable_by(|lhs, rhs| compare_search_entries(lhs, rhs));
    symbols.into_iter().map(|entry| entry.symbol.clone()).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Query {
    query: String,
    lowercased: String,
    path_filter: Vec<String>,
}

impl Query {
    fn new(query: &str) -> Self {
        let (path_filter, query) = parse_query(query);
        let lowercased = query.to_lowercase();
        Self { query, lowercased, path_filter }
    }

    fn search<'a>(&'a self, index: &'a SymbolIndex, mut emit: impl FnMut(&'a SymbolEntry)) {
        let mut stream =
            index.map.search(fst::automaton::Subsequence::new(&self.lowercased)).into_stream();

        while let Some((_, indexed_value)) = stream.next() {
            let (start, end) = SymbolIndex::map_value_to_range(indexed_value);
            for symbol in
                index.symbols[start..end].iter().filter(|entry| self.matches(&entry.symbol))
            {
                emit(symbol);
            }
        }
    }

    fn matches(&self, symbol: &WorkspaceSymbol) -> bool {
        if self.path_filter.is_empty() {
            return true;
        }

        let Some(container_name) = symbol.container_name.as_deref() else {
            return false;
        };

        let mut segments = container_name.split('.');
        self.path_filter
            .iter()
            .all(|filter| segments.any(|segment| subsequence_matches(filter, segment)))
    }
}

fn subsequence_matches(needle: &str, haystack: &str) -> bool {
    let mut needle = needle.bytes();
    let Some(mut next) = needle.next() else {
        return true;
    };

    for byte in haystack.bytes().map(|byte| byte.to_ascii_lowercase()) {
        if byte == next {
            let Some(needle_byte) = needle.next() else {
                return true;
            };
            next = needle_byte;
        }
    }

    false
}

fn parse_query(query: &str) -> (Vec<String>, String) {
    let mut tokens = query
        .split(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '.' | '/' | '\\'))
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();

    let Some(query) = tokens.pop() else {
        return (Vec::new(), String::new());
    };

    (tokens.into_iter().map(str::to_lowercase).collect(), query.to_owned())
}

#[derive(Debug, Default)]
pub struct SymbolIndex {
    symbols: Box<[SymbolEntry]>,
    map: fst::Map<Vec<u8>>,
}

impl PartialEq for SymbolIndex {
    fn eq(&self, other: &Self) -> bool {
        self.symbols == other.symbols
    }
}

impl Eq for SymbolIndex {}

impl SymbolIndex {
    pub(crate) fn for_source_root(
        db: &dyn WorkspaceSymbolIndexDb,
        source_root_id: SourceRootId,
    ) -> Self {
        let source_root = db.source_root(source_root_id);
        let mut symbols = Vec::new();
        for file_id in source_root.iter() {
            symbols.extend(db.file_workspace_symbols(file_id).iter().cloned());
        }
        Self::new(symbols)
    }

    fn new(symbols: Vec<WorkspaceSymbol>) -> Self {
        let mut symbols =
            symbols.into_par_iter().map(SymbolEntry::new).collect::<Vec<_>>().into_boxed_slice();
        symbols.par_sort_by(|lhs, rhs| compare_symbol_entries(lhs, rhs));

        if symbols.is_empty() {
            return Self { symbols, map: fst::Map::default() };
        }

        let mut builder = fst::MapBuilder::memory();
        let mut last_batch_start = 0;
        let mut key = String::new();
        for idx in 0..symbols.len() {
            if let Some(next_symbol) = symbols.get(idx + 1)
                && symbols[last_batch_start].normalized_name == next_symbol.normalized_name
            {
                continue;
            }

            let start = last_batch_start;
            let end = idx + 1;
            last_batch_start = end;
            key.clear();
            key.push_str(&symbols[start].normalized_name);
            let value = Self::range_to_map_value(start, end);
            builder.insert(key.as_str(), value).unwrap();
        }

        let map = builder
            .into_inner()
            .and_then(|mut buf| {
                buf.shrink_to_fit();
                fst::Map::new(buf)
            })
            .unwrap();

        Self { symbols, map }
    }

    fn range_to_map_value(start: usize, end: usize) -> u64 {
        debug_assert!(start <= u32::MAX as usize);
        debug_assert!(end <= u32::MAX as usize);
        ((start as u64) << 32) | end as u64
    }

    fn map_value_to_range(value: u64) -> (usize, usize) {
        let end = value as u32 as usize;
        let start = (value >> 32) as usize;
        (start, end)
    }
}

pub(crate) fn file_symbols(db: &dyn HirDb, file_id: FileId) -> Arc<[WorkspaceSymbol]> {
    let mut symbols = Vec::new();
    for symbol in document_symbols::document_symbols(db, file_id) {
        collect_symbol(file_id, symbol, &mut symbols);
    }
    symbols.into()
}

fn unique_source_root_ids(db: &RootDb, file_ids: Vec<FileId>) -> Vec<SourceRootId> {
    let mut root_ids =
        file_ids.into_iter().map(|file_id| db.source_root_id(file_id)).collect::<Vec<_>>();
    root_ids.sort_unstable();
    root_ids.dedup();
    root_ids
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SymbolEntry {
    symbol: WorkspaceSymbol,
    normalized_name: String,
}

impl SymbolEntry {
    fn new(symbol: WorkspaceSymbol) -> Self {
        let normalized_name = symbol.name.to_lowercase();
        Self { symbol, normalized_name }
    }
}

#[derive(Clone, Copy)]
struct RankedSymbol<'a>(&'a SymbolEntry);

impl PartialEq for RankedSymbol<'_> {
    fn eq(&self, other: &Self) -> bool {
        compare_search_entries(self.0, other.0) == Ordering::Equal
    }
}

impl Eq for RankedSymbol<'_> {}

impl PartialOrd for RankedSymbol<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RankedSymbol<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        compare_search_entries(self.0, other.0)
    }
}

fn compare_search_entries(lhs: &SymbolEntry, rhs: &SymbolEntry) -> Ordering {
    lhs.symbol
        .file_id
        .0
        .cmp(&rhs.symbol.file_id.0)
        .then_with(|| lhs.symbol.focus_range.start().cmp(&rhs.symbol.focus_range.start()))
        .then_with(|| lhs.normalized_name.cmp(&rhs.normalized_name))
        .then_with(|| lhs.symbol.name.cmp(&rhs.symbol.name))
}

fn compare_symbol_entries(lhs: &SymbolEntry, rhs: &SymbolEntry) -> Ordering {
    lhs.normalized_name
        .cmp(&rhs.normalized_name)
        .then_with(|| lhs.symbol.file_id.0.cmp(&rhs.symbol.file_id.0))
        .then_with(|| lhs.symbol.focus_range.start().cmp(&rhs.symbol.focus_range.start()))
        .then_with(|| lhs.symbol.name.cmp(&rhs.symbol.name))
}

fn collect_symbol(file_id: FileId, symbol: DocumentSymbol, symbols: &mut Vec<WorkspaceSymbol>) {
    symbols.push(WorkspaceSymbol {
        file_id,
        name: symbol.name,
        focus_range: symbol.focus_range,
        full_range: symbol.full_range,
        kind: symbol.kind,
        container_name: symbol.container_name,
    });

    for child in symbol.children {
        collect_symbol(file_id, child, symbols);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_parser_splits_path_filter_from_item_query() {
        let query = Query::new("top::inner sig");
        assert_eq!(query.path_filter, vec!["top", "inner"]);
        assert_eq!(query.query, "sig");
    }

    #[test]
    fn query_matches_qualified_symbols() {
        let query = Query::new("top sig");
        assert!(query.matches(&symbol("signal", Some("top"))));
        assert!(!query.matches(&symbol("signal", Some("child"))));
    }

    #[test]
    fn symbol_index_groups_case_insensitive_names() {
        let index = SymbolIndex::new(vec![
            symbol("Top", None),
            symbol("top", Some("pkg")),
            symbol("child", None),
        ]);
        assert_eq!(index.map.len(), 2);
    }

    fn symbol(name: &str, container_name: Option<&str>) -> WorkspaceSymbol {
        WorkspaceSymbol {
            file_id: FileId(0),
            name: name.to_owned(),
            focus_range: TextRange::empty(0.into()),
            full_range: TextRange::empty(0.into()),
            kind: SymbolKind::Module,
            container_name: container_name.map(str::to_owned),
        }
    }
}
