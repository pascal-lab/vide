use hir::{
    base_db::source_db::SourceDb,
    db::HirDb,
    file::HirFileId,
    hir_def::{file::FileItem, module::ModuleId},
};
use itertools::Itertools;
use vfs::FileId;

use crate::{
    FilePosition, FileRange, RangeInfo,
    db::root_db::RootDb,
    facts::symbol::{SymbolId, SymbolInfo},
    goto_definition,
    navigation_target::NavTarget,
    references::{self, References, ReferencesConfig},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub(crate) enum RelationKind {
    Defines,
    References,
    Contains,
    MemberOf,
    Instantiates,
    Imports,
    ExpandsFrom,
    Includes,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Relation {
    pub kind: RelationKind,
    pub source: SymbolId,
    pub target: SymbolId,
    pub range: FileRange,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) enum RelationQuery {
    Incoming { target: SymbolId, kind: RelationKind, config: ReferencesConfig },
    Outgoing { source: SymbolId, kind: RelationKind, config: ReferencesConfig },
    Workspace { kind: RelationKind, config: ReferencesConfig },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RelationSet {
    pub relations: Vec<Relation>,
}

pub(crate) struct RelationFacts<'db> {
    db: &'db RootDb,
}

impl<'db> RelationFacts<'db> {
    pub(crate) fn new(db: &'db RootDb) -> Self {
        Self { db }
    }

    pub(crate) fn definition_targets(
        &self,
        position: FilePosition,
    ) -> Option<RangeInfo<Vec<NavTarget>>> {
        goto_definition::goto_definition(self.db, position)
    }

    pub(crate) fn references(
        &self,
        position: FilePosition,
        config: ReferencesConfig,
    ) -> Option<Vec<References>> {
        references::references(self.db, position, config)
    }

    pub(crate) fn reference_ranges(
        &self,
        position: FilePosition,
        config: ReferencesConfig,
    ) -> Vec<FileRange> {
        self.references(position, config)
            .into_iter()
            .flatten()
            .flat_map(|References { refs, .. }| {
                refs.into_iter().flat_map(|(file_id, refs)| {
                    refs.into_iter().map(move |(range, _)| FileRange { file_id, range })
                })
            })
            .unique()
            .collect()
    }

    pub(crate) fn relations(&self, query: RelationQuery) -> RelationSet {
        match query {
            RelationQuery::Incoming { target, kind, config } => {
                let mut set = self.workspace_relations(kind, config);
                set.relations.retain(|relation| relation.target == target);
                set
            }
            RelationQuery::Outgoing { source, kind, config } => {
                let mut set = self.workspace_relations(kind, config);
                set.relations.retain(|relation| relation.source == source);
                set
            }
            RelationQuery::Workspace { kind, config } => self.workspace_relations(kind, config),
        }
    }

    pub(crate) fn symbol(&self, id: SymbolId) -> Option<SymbolInfo> {
        id.info(self.db)
    }

    pub(crate) fn definition_symbols(&self, position: FilePosition) -> Option<Vec<SymbolInfo>> {
        let nav_info = self.definition_targets(position)?;
        let module_symbols = self.module_symbols();
        let symbols = nav_info
            .info
            .into_iter()
            .filter_map(|target| {
                module_symbols.iter().find(|symbol| nav_matches_symbol(&target, symbol)).cloned()
            })
            .unique_by(|symbol| symbol.id)
            .collect_vec();
        (!symbols.is_empty()).then_some(symbols)
    }

    pub(crate) fn module_symbol_for_item(&self, item: CallSymbolKey) -> Option<SymbolInfo> {
        self.module_symbols().into_iter().find(|symbol| {
            symbol.definition_range == Some(item.full_range)
                && symbol.selection_range == Some(item.selection_range)
        })
    }

    fn workspace_relations(&self, kind: RelationKind, config: ReferencesConfig) -> RelationSet {
        match kind {
            RelationKind::Instantiates => self.instantiation_relations(config),
            _ => RelationSet::default(),
        }
    }

    fn instantiation_relations(&self, config: ReferencesConfig) -> RelationSet {
        let modules = self.module_symbols();
        let mut relations = Vec::new();

        for target in &modules {
            let Some(selection_range) = target.selection_range else {
                continue;
            };
            let position = FilePosition {
                file_id: selection_range.file_id,
                offset: selection_range.range.start(),
            };

            for reference in self.reference_ranges(position, config.clone()) {
                if reference == selection_range {
                    continue;
                }
                let Some(source) = enclosing_module_symbol(&modules, reference) else {
                    continue;
                };
                if source.id == target.id {
                    continue;
                }
                relations.push(Relation {
                    kind: RelationKind::Instantiates,
                    source: source.id,
                    target: target.id,
                    range: reference,
                });
            }
        }

        RelationSet { relations: relations.into_iter().unique().collect() }
    }

    fn module_symbols(&self) -> Vec<SymbolInfo> {
        let mut symbols = Vec::new();
        let mut file_ids = self.file_ids();
        file_ids.sort_unstable_by_key(|file_id| file_id.0);
        file_ids.dedup();

        for file_id in file_ids {
            let hir_file_id = HirFileId::File(file_id);
            let (_file, src_map) = self.db.hir_file_with_source_map(hir_file_id);
            for item in src_map.items.iter() {
                if let FileItem::LocalModuleId(module_id) = *item {
                    let module_id = ModuleId::new(hir_file_id, module_id);
                    if let Some(symbol) = SymbolId::ModuleId(module_id).info(self.db) {
                        symbols.push(symbol);
                    }
                }
            }
        }

        symbols
    }

    pub(crate) fn file_ids(&self) -> Vec<FileId> {
        self.db.files().iter().copied().collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CallSymbolKey {
    pub full_range: FileRange,
    pub selection_range: FileRange,
}

fn nav_matches_symbol(nav: &NavTarget, symbol: &SymbolInfo) -> bool {
    symbol.kind == nav.kind.unwrap_or(symbol.kind)
        && symbol.definition_range
            == Some(FileRange { file_id: nav.file_id, range: nav.full_range })
        && symbol.selection_range
            == Some(FileRange { file_id: nav.file_id, range: nav.focus_or_full_range() })
}

fn enclosing_module_symbol(symbols: &[SymbolInfo], range: FileRange) -> Option<&SymbolInfo> {
    symbols
        .iter()
        .filter(|symbol| {
            let Some(definition_range) = symbol.definition_range else {
                return false;
            };
            definition_range.file_id == range.file_id
                && range_contains_range(definition_range.range, range.range)
        })
        .min_by_key(|symbol| {
            symbol.definition_range.map(|range| range.range.len()).unwrap_or_default()
        })
}

fn range_contains_range(
    container: utils::text_edit::TextRange,
    range: utils::text_edit::TextRange,
) -> bool {
    container.start() <= range.start() && range.end() <= container.end()
}
