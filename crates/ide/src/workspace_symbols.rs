use hir::{
    base_db::{
        source_db::{SourceDb, SourceRootDb},
        source_root::SourceRootId,
    },
    db::HirDb,
};
use index::{
    FileIndex, Occurrence, OccurrenceRole, ProjectIndex, Symbol, SymbolId, SymbolKind,
    SymbolNamespace, SymbolPath, SymbolPathComponent, WorkspaceSymbolQuery,
};
use semantics::Semantics;
use smol_str::SmolStr;
use syntax::{has_text_range::HasTextRange, token::TokenKindExt};
use utils::line_index::TextRange;
use vfs::FileId;

use crate::{
    db::{root_db::RootDb, workspace_symbol_index_db::WorkspaceSymbolIndexDb},
    definitions::{DefinitionClass, DefinitionOrigin},
    document_symbols::{self, DocumentSymbol},
    source_targets::{SourceTargetRequestCache, source_target_at_offset_with_cache},
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
            source_root_project_index(db, source_root_id)
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

pub(crate) fn source_root_project_index(db: &RootDb, source_root_id: SourceRootId) -> ProjectIndex {
    let source_root = db.source_root(source_root_id);
    ProjectIndex::from_files(source_root.iter().map(|file_id| {
        let mut index = WorkspaceSymbolIndexDb::file_index(db, file_id).as_ref().clone();
        collect_source_occurrences(db, file_id, &mut index);
        index
    }))
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
        kind: symbol.kind,
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

fn collect_source_occurrences(db: &RootDb, file_id: FileId, index: &mut FileIndex) {
    let sema = Semantics::new(db);
    let parsed_file = sema.parse_file(file_id);
    let Some(root) = parsed_file.root() else {
        return;
    };
    let text = db.file_text(file_id);
    let mut cache = SourceTargetRequestCache::default();
    for offset in identifier_offsets(&text) {
        let Some(target) = source_target_at_offset_with_cache(
            db,
            file_id,
            root,
            offset,
            workspace_symbol_token_precedence,
            &mut cache,
        )
        .and_then(|target| target.resolved()) else {
            continue;
        };

        for token in target.into_tokens() {
            let Some(range) = token.text_range() else {
                continue;
            };
            let Some(definition_class) = DefinitionClass::resolve(&sema, file_id.into(), token)
            else {
                continue;
            };
            for origin in definition_class.origins() {
                if origin.name_range(db).is_some_and(|name_range| {
                    name_range.file_id.file_id() == file_id && name_range.value == range
                }) {
                    continue;
                }
                let Some(symbol) = symbol_id_for_origin(db, origin) else {
                    continue;
                };
                index.occurrences.push(Occurrence {
                    symbol,
                    file_id,
                    range,
                    role: OccurrenceRole::Reference,
                    container: None,
                    syntax_kind: None,
                });
            }
        }
    }
}

fn workspace_symbol_token_precedence(kind: syntax::TokenKind) -> usize {
    usize::from(kind.name_like())
}

fn identifier_offsets(text: &str) -> impl Iterator<Item = utils::line_index::TextSize> + '_ {
    let mut prev_ident = false;
    text.char_indices().filter_map(move |(idx, ch)| {
        let is_ident = ch == '_' || ch.is_ascii_alphanumeric();
        let is_start = is_ident && !prev_ident && (ch == '_' || ch.is_ascii_alphabetic());
        prev_ident = is_ident;
        is_start.then_some((idx as u32).into())
    })
}

pub(crate) fn symbol_id_for_origin(db: &dyn HirDb, origin: DefinitionOrigin) -> Option<SymbolId> {
    let name = origin.name(db)?;
    let kind = symbol_kind_for_origin(origin);
    Some(symbol_id(&name, kind, &[]))
}

fn symbol_kind_for_origin(origin: DefinitionOrigin) -> SymbolKind {
    match origin {
        DefinitionOrigin::ModuleId(_) => SymbolKind::Module,
        DefinitionOrigin::Config(_) => SymbolKind::Config,
        DefinitionOrigin::Library(_) => SymbolKind::Library,
        DefinitionOrigin::Udp(_) => SymbolKind::Primitive,
        DefinitionOrigin::BlockId(_) => SymbolKind::Block,
        DefinitionOrigin::GenerateBlockId(_) => SymbolKind::Generate,
        DefinitionOrigin::SubroutineId(_) => SymbolKind::Fn,
        DefinitionOrigin::SubroutinePort(_) => SymbolKind::PortDecl,
        DefinitionOrigin::NonAnsiPort(_) => SymbolKind::NonAnsiPortLabel,
        DefinitionOrigin::Decl(_) => SymbolKind::DataDecl,
        DefinitionOrigin::Typedef(_) => SymbolKind::Typedef,
        DefinitionOrigin::Instance(_) => SymbolKind::Instance,
        DefinitionOrigin::Stmt(_) => SymbolKind::Stmt,
    }
}

fn symbol_id(name: &SmolStr, kind: SymbolKind, container_path: &[SmolStr]) -> SymbolId {
    let mut components = container_path
        .iter()
        .map(|component| SymbolPathComponent::Module(component.clone()))
        .collect::<Vec<_>>();
    components.push(symbol_path_component(name.clone(), kind));
    SymbolId::new(SymbolNamespace::Work, SymbolPath::new(components), kind)
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

    #[test]
    fn project_index_resolves_cross_file_module_instantiation_occurrence() {
        use hir::base_db::{change::Change, source_root::SourceRoot};
        use triomphe::Arc;
        use utils::lines::LineEnding;
        use vfs::{ChangeKind, ChangedFile, FileSet, VfsPath};

        let child_file = FileId(0);
        let top_file = FileId(1);
        let child_text = "module child; endmodule\n";
        let top_text = "module top;\n  child u_child();\nendmodule\n";
        let mut files = FileSet::default();
        files.insert(child_file, VfsPath::new_virtual_path("/child.sv".to_owned()));
        files.insert(top_file, VfsPath::new_virtual_path("/top.sv".to_owned()));
        let root = SourceRoot::new_local(files);
        let mut change = Change::new();
        change.set_roots(vec![root]);
        change.add_changed_file(ChangedFile {
            file_id: child_file,
            change_kind: ChangeKind::Create(Arc::from(child_text), LineEnding::Unix),
        });
        change.add_changed_file(ChangedFile {
            file_id: top_file,
            change_kind: ChangeKind::Create(Arc::from(top_text), LineEnding::Unix),
        });

        let mut host = crate::analysis_host::AnalysisHost::default();
        host.apply_change(change);
        let db = host.raw_db();
        let root_id = db.source_root_id(top_file);
        let project_index = source_root_project_index(db, root_id);
        let offset = top_text.find("child u_child").unwrap() as u32;

        let definitions = project_index.definitions_for_occurrence(top_file, offset.into());

        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].name, "child");
        assert_eq!(definitions[0].file_id, child_file);
    }
}
