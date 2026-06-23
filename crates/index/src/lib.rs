//! Project-wide symbol and occurrence index model for IDE queries.
//!
//! This crate owns stable indexing data structures. It is intentionally
//! independent from LSP types and from IDE feature implementations. The first
//! implementation is an in-memory live index over `FileIndex` values; later
//! phases can back the same query surface with salsa or persistent storage.

use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use syntax::{SyntaxKind, ast, match_ast_kind};
use utils::line_index::TextRange;
use vfs::FileId;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolId {
    pub namespace: SymbolNamespace,
    pub path: SymbolPath,
    pub kind: SymbolKind,
}

impl SymbolId {
    pub fn new(namespace: SymbolNamespace, path: SymbolPath, kind: SymbolKind) -> Self {
        Self { namespace, path, kind }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SymbolNamespace {
    Work,
    Library(SmolStr),
    Package(SmolStr),
    Macro,
    Builtin,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct SymbolPath {
    components: Vec<SymbolPathComponent>,
}

impl SymbolPath {
    pub fn new(components: Vec<SymbolPathComponent>) -> Self {
        Self { components }
    }

    pub fn single(component: SymbolPathComponent) -> Self {
        Self { components: vec![component] }
    }

    pub fn components(&self) -> &[SymbolPathComponent] {
        &self.components
    }

    pub fn display_name(&self) -> Option<&str> {
        self.components.last().map(SymbolPathComponent::name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SymbolPathComponent {
    Module(SmolStr),
    Interface(SmolStr),
    Package(SmolStr),
    Class(SmolStr),
    Instance(SmolStr),
    GenerateBlock(SmolStr),
    Port(SmolStr),
    Signal(SmolStr),
    Typedef(SmolStr),
    Function(SmolStr),
    Task(SmolStr),
    Macro(SmolStr),
}

impl SymbolPathComponent {
    pub fn name(&self) -> &str {
        match self {
            SymbolPathComponent::Module(name)
            | SymbolPathComponent::Interface(name)
            | SymbolPathComponent::Package(name)
            | SymbolPathComponent::Class(name)
            | SymbolPathComponent::Instance(name)
            | SymbolPathComponent::GenerateBlock(name)
            | SymbolPathComponent::Port(name)
            | SymbolPathComponent::Signal(name)
            | SymbolPathComponent::Typedef(name)
            | SymbolPathComponent::Function(name)
            | SymbolPathComponent::Task(name)
            | SymbolPathComponent::Macro(name) => name,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SymbolKind {
    Module,
    Config,
    Primitive,
    NonAnsiPortLabel,
    PortDecl,
    ParamDecl,
    NetDecl,
    DataDecl,
    Genvar,
    Specparam,
    Typedef,
    Struct,
    Instance,
    Block,
    Stmt,
    Fn,
    Generate,
    Specify,
    Interface,
    Library,
    Region,
    Macro,
    Unknown,
}

impl SymbolKind {
    pub fn from_syntax_kind(kind: SyntaxKind) -> Self {
        match_ast_kind! { kind,
            ast::ModuleDeclaration where kind == SyntaxKind::MODULE_DECLARATION => SymbolKind::Module,
            ast::ConfigDeclaration => SymbolKind::Config,
            ast::UdpDeclaration => SymbolKind::Primitive,
            ast::NonAnsiPort => SymbolKind::NonAnsiPortLabel,
            ast::PortDeclaration => SymbolKind::PortDecl,
            ast::ParameterDeclaration => SymbolKind::ParamDecl,
            ast::NetDeclaration => SymbolKind::NetDecl,
            ast::DataDeclaration => SymbolKind::DataDecl,
            ast::GenvarDeclaration => SymbolKind::Genvar,
            ast::LibraryDeclaration => SymbolKind::Library,
            ast::SpecparamDeclaration => SymbolKind::Specparam,
            ast::TypedefDeclaration => SymbolKind::Typedef,
            ast::Declarator => SymbolKind::DataDecl,
            ast::HierarchicalInstance => SymbolKind::Instance,
            ast::BlockStatement => SymbolKind::Block,
            ast::Statement => SymbolKind::Stmt,
            ast::FunctionDeclaration => SymbolKind::Fn,
            ast::SpecifyBlock => SymbolKind::Specify,
            _ => SymbolKind::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: SmolStr,
    pub definition: TextRange,
    pub full_range: TextRange,
    pub file_id: FileId,
    pub kind: SymbolKind,
    pub container_name: Option<SmolStr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    pub symbol: SymbolId,
    pub file_id: FileId,
    pub range: TextRange,
    pub role: OccurrenceRole,
    pub container: Option<SymbolId>,
    pub syntax_kind: Option<SyntaxKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OccurrenceRole {
    Definition,
    Reference,
    Read,
    Write,
    Import,
    Include,
    MacroDefinition,
    MacroExpansion,
    Generated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeEdge {
    pub from_file: FileId,
    pub include_range: TextRange,
    pub target_text: SmolStr,
    pub resolved_file: Option<FileId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroOccurrence {
    pub name: SmolStr,
    pub file_id: FileId,
    pub range: TextRange,
    pub role: MacroRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MacroRole {
    Definition,
    Reference,
    Expansion,
    Undef,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DependencyKey {
    File(FileId),
    Symbol(SymbolId),
    Macro(SmolStr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileIndex {
    pub file_id: FileId,
    pub symbols: Vec<Symbol>,
    pub occurrences: Vec<Occurrence>,
    pub includes: Vec<IncludeEdge>,
    pub macro_occurrences: Vec<MacroOccurrence>,
    pub dependencies: Vec<DependencyKey>,
}

impl FileIndex {
    pub fn new(file_id: FileId) -> Self {
        Self {
            file_id,
            symbols: Vec::new(),
            occurrences: Vec::new(),
            includes: Vec::new(),
            macro_occurrences: Vec::new(),
            dependencies: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSymbolQuery {
    item_query: String,
    lowercased_item_query: String,
    path_filter: Vec<String>,
}

impl WorkspaceSymbolQuery {
    pub fn new(query: &str) -> Self {
        let (path_filter, item_query) = parse_workspace_symbol_query(query);
        let lowercased_item_query = item_query.to_lowercase();
        Self { item_query, lowercased_item_query, path_filter }
    }

    pub fn item_query(&self) -> &str {
        &self.item_query
    }

    pub fn path_filter(&self) -> &[String] {
        &self.path_filter
    }

    fn matches(&self, symbol: &Symbol) -> bool {
        if !subsequence_matches(&self.lowercased_item_query, &symbol.name.to_lowercase()) {
            return false;
        }
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

fn parse_workspace_symbol_query(query: &str) -> (Vec<String>, String) {
    let mut tokens = query
        .split(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '.' | '/' | '\\'))
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();

    let Some(query) = tokens.pop() else {
        return (Vec::new(), String::new());
    };

    (tokens.into_iter().map(str::to_lowercase).collect(), query.to_owned())
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

fn compare_workspace_symbols(lhs: &Symbol, rhs: &Symbol) -> std::cmp::Ordering {
    lhs.file_id
        .0
        .cmp(&rhs.file_id.0)
        .then_with(|| lhs.definition.start().cmp(&rhs.definition.start()))
        .then_with(|| lhs.name.cmp(&rhs.name))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectIndex {
    files: FxHashMap<FileId, FileIndex>,
    symbol_definitions: FxHashMap<SymbolId, Vec<Symbol>>,
    symbol_occurrences: FxHashMap<SymbolId, Vec<Occurrence>>,
    file_dependents: FxHashMap<FileId, Vec<FileId>>,
}

impl ProjectIndex {
    pub fn from_files(files: impl IntoIterator<Item = FileIndex>) -> Self {
        let mut index = Self::default();
        for file in files {
            index.insert_file(file);
        }
        index
    }

    pub fn insert_file(&mut self, file: FileIndex) {
        self.remove_file(file.file_id);

        for symbol in &file.symbols {
            self.symbol_definitions.entry(symbol.id.clone()).or_default().push(symbol.clone());
        }

        for occurrence in &file.occurrences {
            self.symbol_occurrences
                .entry(occurrence.symbol.clone())
                .or_default()
                .push(occurrence.clone());
        }

        for dependency in &file.dependencies {
            if let DependencyKey::File(depended_on) = dependency {
                self.file_dependents.entry(*depended_on).or_default().push(file.file_id);
            }
        }

        self.files.insert(file.file_id, file);
    }

    pub fn remove_file(&mut self, file_id: FileId) -> Option<FileIndex> {
        let removed = self.files.remove(&file_id)?;

        for symbol in &removed.symbols {
            if let Some(symbols) = self.symbol_definitions.get_mut(&symbol.id) {
                symbols.retain(|existing| existing.file_id != file_id);
                if symbols.is_empty() {
                    self.symbol_definitions.remove(&symbol.id);
                }
            }
        }

        for occurrence in &removed.occurrences {
            if let Some(occurrences) = self.symbol_occurrences.get_mut(&occurrence.symbol) {
                occurrences.retain(|existing| existing.file_id != file_id);
                if occurrences.is_empty() {
                    self.symbol_occurrences.remove(&occurrence.symbol);
                }
            }
        }

        self.rebuild_file_dependents();
        Some(removed)
    }

    pub fn file_index(&self, file_id: FileId) -> Option<&FileIndex> {
        self.files.get(&file_id)
    }

    pub fn symbol_definitions(&self, symbol: &SymbolId) -> &[Symbol] {
        self.symbol_definitions.get(symbol).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn symbol_occurrences(&self, symbol: &SymbolId) -> &[Occurrence] {
        self.symbol_occurrences.get(symbol).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn occurrences_at(
        &self,
        file_id: FileId,
        offset: utils::line_index::TextSize,
    ) -> Vec<&Occurrence> {
        self.files
            .get(&file_id)
            .into_iter()
            .flat_map(|file| file.occurrences.iter())
            .filter(|occurrence| occurrence.range.contains_inclusive(offset))
            .collect()
    }

    pub fn definitions_for_occurrence(
        &self,
        file_id: FileId,
        offset: utils::line_index::TextSize,
    ) -> Vec<&Symbol> {
        self.occurrences_at(file_id, offset)
            .into_iter()
            .flat_map(|occurrence| self.symbol_definitions(&occurrence.symbol))
            .collect()
    }

    pub fn workspace_symbols(&self, query: &WorkspaceSymbolQuery, limit: usize) -> Vec<&Symbol> {
        let mut symbols = self
            .symbol_definitions
            .values()
            .flat_map(|symbols| symbols.iter())
            .filter(|symbol| query.matches(symbol))
            .collect::<Vec<_>>();
        symbols.sort_by(|lhs, rhs| compare_workspace_symbols(lhs, rhs));
        symbols.truncate(limit);
        symbols
    }

    pub fn files_depending_on(&self, file_id: FileId) -> &[FileId] {
        self.file_dependents.get(&file_id).map(Vec::as_slice).unwrap_or(&[])
    }

    fn rebuild_file_dependents(&mut self) {
        self.file_dependents.clear();
        for file in self.files.values() {
            for dependency in &file.dependencies {
                if let DependencyKey::File(depended_on) = dependency {
                    self.file_dependents.entry(*depended_on).or_default().push(file.file_id);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn top_symbol() -> SymbolId {
        SymbolId::new(
            SymbolNamespace::Work,
            SymbolPath::single(SymbolPathComponent::Module("top".into())),
            SymbolKind::Module,
        )
    }

    #[test]
    fn project_index_answers_symbol_and_occurrence_queries() {
        let symbol = top_symbol();
        let mut file = FileIndex::new(FileId(0));
        file.symbols.push(Symbol {
            id: symbol.clone(),
            name: "top".into(),
            definition: TextRange::new(7.into(), 10.into()),
            full_range: TextRange::new(0.into(), 11.into()),
            file_id: FileId(0),
            kind: SymbolKind::Module,
            container_name: None,
        });
        file.occurrences.push(Occurrence {
            symbol: symbol.clone(),
            file_id: FileId(1),
            range: TextRange::new(0.into(), 3.into()),
            role: OccurrenceRole::Reference,
            container: None,
            syntax_kind: None,
        });
        file.dependencies.push(DependencyKey::File(FileId(2)));

        let index = ProjectIndex::from_files([file]);

        assert_eq!(index.symbol_definitions(&symbol).len(), 1);
        assert_eq!(index.symbol_occurrences(&symbol).len(), 1);
        assert_eq!(index.workspace_symbols(&WorkspaceSymbolQuery::new("to"), 16).len(), 1);
        assert_eq!(index.files_depending_on(FileId(2)), &[FileId(0)]);
    }

    #[test]
    fn inserting_file_replaces_stale_entries() {
        let symbol = top_symbol();
        let mut first = FileIndex::new(FileId(0));
        first.symbols.push(Symbol {
            id: symbol.clone(),
            name: "top".into(),
            definition: TextRange::new(0.into(), 3.into()),
            full_range: TextRange::new(0.into(), 3.into()),
            file_id: FileId(0),
            kind: SymbolKind::Module,
            container_name: None,
        });

        let replacement = FileIndex::new(FileId(0));
        let mut index = ProjectIndex::from_files([first]);
        index.insert_file(replacement);

        assert!(index.symbol_definitions(&symbol).is_empty());
        assert!(index.file_index(FileId(0)).is_some());
    }

    #[test]
    fn workspace_symbol_query_filters_by_container_and_limits_results() {
        let mut file = FileIndex::new(FileId(0));
        for (idx, (name, container)) in
            [("signal", Some("top")), ("signal", Some("child")), ("top", None)]
                .into_iter()
                .enumerate()
        {
            let symbol = SymbolId::new(
                SymbolNamespace::Work,
                SymbolPath::single(SymbolPathComponent::Signal(name.into())),
                SymbolKind::DataDecl,
            );
            file.symbols.push(Symbol {
                id: symbol,
                name: name.into(),
                definition: TextRange::new((idx as u32).into(), (idx as u32 + 1).into()),
                file_id: FileId(0),
                full_range: TextRange::new((idx as u32).into(), (idx as u32 + 1).into()),
                kind: SymbolKind::DataDecl,
                container_name: container.map(Into::into),
            });
        }

        let index = ProjectIndex::from_files([file]);
        let matches = index.workspace_symbols(&WorkspaceSymbolQuery::new("top sig"), 16);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "signal");
        assert_eq!(matches[0].container_name.as_deref(), Some("top"));
    }

    #[test]
    fn definitions_for_occurrence_resolves_reference_to_symbol_definition() {
        let symbol = top_symbol();
        let mut definition_file = FileIndex::new(FileId(0));
        definition_file.symbols.push(Symbol {
            id: symbol.clone(),
            name: "top".into(),
            definition: TextRange::new(7.into(), 10.into()),
            full_range: TextRange::new(0.into(), 11.into()),
            file_id: FileId(0),
            kind: SymbolKind::Module,
            container_name: None,
        });
        let mut reference_file = FileIndex::new(FileId(1));
        reference_file.occurrences.push(Occurrence {
            symbol: symbol.clone(),
            file_id: FileId(1),
            range: TextRange::new(20.into(), 23.into()),
            role: OccurrenceRole::Reference,
            container: None,
            syntax_kind: None,
        });

        let index = ProjectIndex::from_files([definition_file, reference_file]);
        let definitions = index.definitions_for_occurrence(FileId(1), 21.into());

        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].name, "top");
    }
}
