use hir::base_db::{source_db::SourceRootDb, source_root::SourceRootId};
use index::{Symbol, SymbolKind, WorkspaceSymbolQuery};
use utils::line_index::TextRange;
use vfs::FileId;

use crate::{db::root_db::RootDb, indexing::ProjectIndexDatabase};

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

impl From<&Symbol> for WorkspaceSymbol {
    fn from(symbol: &Symbol) -> Self {
        Self {
            file_id: symbol.file_id,
            name: symbol.name.to_string(),
            focus_range: symbol.definition,
            full_range: symbol.full_range,
            kind: symbol.kind,
            container_name: symbol.container_name.as_ref().map(ToString::to_string),
        }
    }
}

pub(crate) fn workspace_symbols(
    db: &RootDb,
    query: &str,
    file_ids: Vec<FileId>,
) -> Vec<WorkspaceSymbol> {
    let query = WorkspaceSymbolQuery::new(query);
    let root_ids = unique_source_root_ids(db, file_ids);
    let mut symbols = root_ids
        .into_iter()
        .flat_map(|source_root_id| {
            db.source_root_project_index(source_root_id)
                .workspace_symbols(&query, WORKSPACE_SYMBOL_LIMIT)
                .into_iter()
                .map(WorkspaceSymbol::from)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    symbols.sort_unstable_by(compare_workspace_symbols);
    symbols.truncate(WORKSPACE_SYMBOL_LIMIT);
    symbols
}

fn unique_source_root_ids(db: &RootDb, file_ids: Vec<FileId>) -> Vec<SourceRootId> {
    let mut root_ids =
        file_ids.into_iter().map(|file_id| db.source_root_id(file_id)).collect::<Vec<_>>();
    root_ids.sort_unstable();
    root_ids.dedup();
    root_ids
}

fn compare_workspace_symbols(lhs: &WorkspaceSymbol, rhs: &WorkspaceSymbol) -> std::cmp::Ordering {
    lhs.file_id
        .0
        .cmp(&rhs.file_id.0)
        .then_with(|| lhs.focus_range.start().cmp(&rhs.focus_range.start()))
        .then_with(|| lhs.name.cmp(&rhs.name))
}
