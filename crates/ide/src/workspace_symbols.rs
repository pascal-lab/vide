use hir::{
    base_db::{source_db::SourceRootDb, source_root::SourceRootId},
    db::HirDb,
};
use index::{
    FileIndex, ProjectIndex, Symbol, SymbolId, SymbolNamespace, SymbolPath, SymbolPathComponent,
    WorkspaceSymbolQuery,
};
use smol_str::SmolStr;
use utils::line_index::TextRange;
use vfs::FileId;

use crate::{
    SymbolKind,
    db::{root_db::RootDb, workspace_symbol_index_db::WorkspaceSymbolIndexDb},
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

impl From<&Symbol> for WorkspaceSymbol {
    fn from(symbol: &Symbol) -> Self {
        Self {
            file_id: symbol.file_id,
            name: symbol.name.to_string(),
            focus_range: symbol.definition,
            full_range: symbol.full_range,
            kind: from_index_symbol_kind(symbol.kind),
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
            WorkspaceSymbolIndexDb::source_root_project_index(db, source_root_id)
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

pub(crate) fn source_root_project_index(
    db: &dyn WorkspaceSymbolIndexDb,
    source_root_id: SourceRootId,
) -> ProjectIndex {
    let source_root = db.source_root(source_root_id);
    ProjectIndex::from_files(
        source_root.iter().map(|file_id| db.file_index(file_id).as_ref().clone()),
    )
}

pub(crate) fn file_index(db: &dyn HirDb, file_id: FileId) -> FileIndex {
    let mut index = FileIndex::new(file_id);
    for symbol in document_symbols::document_symbols(db, file_id) {
        collect_symbol(file_id, symbol, Vec::new(), &mut index);
    }
    index
}

fn unique_source_root_ids(db: &RootDb, file_ids: Vec<FileId>) -> Vec<SourceRootId> {
    let mut root_ids =
        file_ids.into_iter().map(|file_id| db.source_root_id(file_id)).collect::<Vec<_>>();
    root_ids.sort_unstable();
    root_ids.dedup();
    root_ids
}

fn collect_symbol(
    file_id: FileId,
    symbol: DocumentSymbol,
    container_path: Vec<SmolStr>,
    index: &mut FileIndex,
) {
    let name = SmolStr::new(symbol.name.as_str());
    let id = symbol_id(&name, symbol.kind, &container_path);
    let mut child_container_path = container_path;
    child_container_path.push(name.clone());

    index.symbols.push(Symbol {
        id: id.clone(),
        name,
        definition: symbol.focus_range,
        full_range: symbol.full_range,
        file_id,
        kind: to_index_symbol_kind(symbol.kind),
        container_name: symbol.container_name.map(SmolStr::from),
    });

    index.occurrences.push(index::Occurrence {
        symbol: id,
        file_id,
        range: symbol.focus_range,
        role: index::OccurrenceRole::Definition,
        container: None,
        syntax_kind: None,
    });

    for child in symbol.children {
        collect_symbol(file_id, child, child_container_path.clone(), index);
    }
}

fn symbol_id(name: &SmolStr, kind: SymbolKind, container_path: &[SmolStr]) -> SymbolId {
    let mut components = container_path
        .iter()
        .map(|component| SymbolPathComponent::Module(component.clone()))
        .collect::<Vec<_>>();
    components.push(symbol_path_component(name.clone(), kind));
    SymbolId::new(SymbolNamespace::Work, SymbolPath::new(components), to_index_symbol_kind(kind))
}

fn symbol_path_component(name: SmolStr, kind: SymbolKind) -> SymbolPathComponent {
    match kind {
        SymbolKind::Module => SymbolPathComponent::Module(name),
        SymbolKind::Interface => SymbolPathComponent::Interface(name),
        SymbolKind::Instance => SymbolPathComponent::Instance(name),
        SymbolKind::Typedef | SymbolKind::Struct => SymbolPathComponent::Typedef(name),
        SymbolKind::Fn => SymbolPathComponent::Function(name),
        SymbolKind::NonAnsiPortLabel | SymbolKind::PortDecl => SymbolPathComponent::Port(name),
        _ => SymbolPathComponent::Signal(name),
    }
}

fn to_index_symbol_kind(kind: SymbolKind) -> index::SymbolKind {
    match kind {
        SymbolKind::Module => index::SymbolKind::Module,
        SymbolKind::Config => index::SymbolKind::Config,
        SymbolKind::Primitive => index::SymbolKind::Primitive,
        SymbolKind::NonAnsiPortLabel => index::SymbolKind::NonAnsiPortLabel,
        SymbolKind::PortDecl => index::SymbolKind::PortDecl,
        SymbolKind::ParamDecl => index::SymbolKind::ParamDecl,
        SymbolKind::NetDecl => index::SymbolKind::NetDecl,
        SymbolKind::DataDecl => index::SymbolKind::DataDecl,
        SymbolKind::Genvar => index::SymbolKind::Genvar,
        SymbolKind::Specparam => index::SymbolKind::Specparam,
        SymbolKind::Typedef => index::SymbolKind::Typedef,
        SymbolKind::Struct => index::SymbolKind::Struct,
        SymbolKind::Instance => index::SymbolKind::Instance,
        SymbolKind::Block => index::SymbolKind::Block,
        SymbolKind::Stmt => index::SymbolKind::Stmt,
        SymbolKind::Fn => index::SymbolKind::Fn,
        SymbolKind::Generate => index::SymbolKind::Generate,
        SymbolKind::Specify => index::SymbolKind::Specify,
        SymbolKind::Interface => index::SymbolKind::Interface,
        SymbolKind::Library => index::SymbolKind::Library,
        SymbolKind::Region => index::SymbolKind::Region,
        SymbolKind::Unknown => index::SymbolKind::Unknown,
    }
}

fn from_index_symbol_kind(kind: index::SymbolKind) -> SymbolKind {
    match kind {
        index::SymbolKind::Module => SymbolKind::Module,
        index::SymbolKind::Config => SymbolKind::Config,
        index::SymbolKind::Primitive => SymbolKind::Primitive,
        index::SymbolKind::NonAnsiPortLabel => SymbolKind::NonAnsiPortLabel,
        index::SymbolKind::PortDecl => SymbolKind::PortDecl,
        index::SymbolKind::ParamDecl => SymbolKind::ParamDecl,
        index::SymbolKind::NetDecl => SymbolKind::NetDecl,
        index::SymbolKind::DataDecl => SymbolKind::DataDecl,
        index::SymbolKind::Genvar => SymbolKind::Genvar,
        index::SymbolKind::Specparam => SymbolKind::Specparam,
        index::SymbolKind::Typedef => SymbolKind::Typedef,
        index::SymbolKind::Struct => SymbolKind::Struct,
        index::SymbolKind::Instance => SymbolKind::Instance,
        index::SymbolKind::Block => SymbolKind::Block,
        index::SymbolKind::Stmt => SymbolKind::Stmt,
        index::SymbolKind::Fn => SymbolKind::Fn,
        index::SymbolKind::Generate => SymbolKind::Generate,
        index::SymbolKind::Specify => SymbolKind::Specify,
        index::SymbolKind::Interface => SymbolKind::Interface,
        index::SymbolKind::Library => SymbolKind::Library,
        index::SymbolKind::Region => SymbolKind::Region,
        index::SymbolKind::Macro => SymbolKind::Unknown,
        index::SymbolKind::Unknown => SymbolKind::Unknown,
    }
}

fn compare_workspace_symbols(lhs: &WorkspaceSymbol, rhs: &WorkspaceSymbol) -> std::cmp::Ordering {
    lhs.file_id
        .0
        .cmp(&rhs.file_id.0)
        .then_with(|| lhs.focus_range.start().cmp(&rhs.focus_range.start()))
        .then_with(|| lhs.name.cmp(&rhs.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_document_symbol_tree_to_file_index() {
        let symbol = DocumentSymbol {
            name: "top".to_owned(),
            focus_range: TextRange::new(7.into(), 10.into()),
            full_range: TextRange::new(0.into(), 20.into()),
            kind: SymbolKind::Module,
            detail: None,
            container_name: None,
            children: vec![DocumentSymbol {
                name: "sig".to_owned(),
                focus_range: TextRange::new(15.into(), 18.into()),
                full_range: TextRange::new(12.into(), 19.into()),
                kind: SymbolKind::DataDecl,
                detail: None,
                container_name: Some("top".to_owned()),
                children: Vec::new(),
            }],
        };
        let mut index = FileIndex::new(FileId(0));
        collect_symbol(FileId(0), symbol, Vec::new(), &mut index);

        assert_eq!(index.symbols.len(), 2);
        assert_eq!(index.occurrences.len(), 2);
        assert_eq!(index.symbols[1].container_name.as_deref(), Some("top"));
    }
}
