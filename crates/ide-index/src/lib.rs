//! Project-wide symbol and occurrence index model for IDE queries.
//!
//! This crate owns stable indexing data structures. It is intentionally
//! independent from LSP types and from IDE feature implementations. The first
//! implementation is an in-memory live index over `FileIndex` values; later
//! phases can back the same query surface with salsa or persistent storage.

use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use syntax::SyntaxKind;
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
    Interface,
    Package,
    Class,
    Instance,
    GenerateBlock,
    Port,
    Signal,
    Typedef,
    Function,
    Task,
    Macro,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: SmolStr,
    pub definition: TextRange,
    pub file_id: FileId,
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

#[derive(Debug, Clone, Default)]
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

    pub fn workspace_symbols(&self, query: &str) -> Vec<&Symbol> {
        let query = query.to_lowercase();
        let mut symbols = self
            .symbol_definitions
            .values()
            .flat_map(|symbols| symbols.iter())
            .filter(|symbol| symbol.name.to_lowercase().contains(&query))
            .collect::<Vec<_>>();
        symbols.sort_by_key(|symbol| (symbol.name.clone(), symbol.file_id));
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
            file_id: FileId(0),
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
        assert_eq!(index.workspace_symbols("to").len(), 1);
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
            file_id: FileId(0),
        });

        let replacement = FileIndex::new(FileId(0));
        let mut index = ProjectIndex::from_files([first]);
        index.insert_file(replacement);

        assert!(index.symbol_definitions(&symbol).is_empty());
        assert!(index.file_index(FileId(0)).is_some());
    }
}
