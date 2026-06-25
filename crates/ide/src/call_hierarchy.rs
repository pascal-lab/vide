use itertools::Itertools;

use crate::{
    FilePosition, FileRange, SymbolKind,
    db::root_db::RootDb,
    facts::{
        SemanticFacts,
        relation::{CallSymbolKey, RelationFacts, RelationKind, RelationQuery},
        symbol::{SymbolId, SymbolInfo},
    },
    references::ReferencesConfig,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallHierarchyItem {
    pub symbol: Option<SymbolId>,
    pub name: String,
    pub kind: SymbolKind,
    pub detail: Option<String>,
    pub full_range: FileRange,
    pub selection_range: FileRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingCall {
    pub from: CallHierarchyItem,
    pub from_ranges: Vec<FileRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingCall {
    pub to: CallHierarchyItem,
    pub from_ranges: Vec<FileRange>,
}

pub(crate) fn prepare(db: &RootDb, position: FilePosition) -> Option<Vec<CallHierarchyItem>> {
    let facts = SemanticFacts::new(db);
    let relations = facts.relations();
    let items = relations
        .definition_symbols(position)?
        .into_iter()
        .filter_map(|symbol| call_hierarchy_item_for_symbol(&relations, symbol))
        .unique_by(call_hierarchy_item_key)
        .collect_vec();

    (!items.is_empty()).then_some(items)
}

pub(crate) fn incoming(
    db: &RootDb,
    target: CallHierarchyItem,
    config: ReferencesConfig,
) -> Option<Vec<IncomingCall>> {
    let facts = SemanticFacts::new(db);
    let relations = facts.relations();
    let target = symbol_for_item(&relations, &target)?;
    let relation_set = relations.relations(RelationQuery::Incoming {
        target: target.id,
        kind: RelationKind::Instantiates,
        config,
    });
    let mut groups = Vec::<(SymbolId, Vec<FileRange>)>::new();
    for relation in relation_set.relations {
        push_call_range(&mut groups, relation.source, relation.range);
    }

    let calls: Vec<_> = groups
        .into_iter()
        .filter_map(|(source, from_ranges)| {
            let from = call_hierarchy_item_for_symbol(&relations, relations.symbol(source)?)?;
            Some(IncomingCall { from, from_ranges })
        })
        .collect();
    (!calls.is_empty()).then_some(calls)
}

pub(crate) fn outgoing(
    db: &RootDb,
    caller: CallHierarchyItem,
    config: ReferencesConfig,
) -> Option<Vec<OutgoingCall>> {
    let facts = SemanticFacts::new(db);
    let relations = facts.relations();
    let caller = symbol_for_item(&relations, &caller)?;
    let relation_set = relations.relations(RelationQuery::Outgoing {
        source: caller.id,
        kind: RelationKind::Instantiates,
        config,
    });
    let mut groups = Vec::<(SymbolId, Vec<FileRange>)>::new();
    for relation in relation_set.relations {
        push_call_range(&mut groups, relation.target, relation.range);
    }

    let calls: Vec<_> = groups
        .into_iter()
        .filter_map(|(target, from_ranges)| {
            let to = call_hierarchy_item_for_symbol(&relations, relations.symbol(target)?)?;
            Some(OutgoingCall { to, from_ranges })
        })
        .collect();
    (!calls.is_empty()).then_some(calls)
}

fn symbol_for_item(relations: &RelationFacts<'_>, item: &CallHierarchyItem) -> Option<SymbolInfo> {
    if let Some(symbol) = item.symbol {
        return relations.symbol(symbol);
    }
    relations.module_symbol_for_item(CallSymbolKey {
        full_range: item.full_range,
        selection_range: item.selection_range,
    })
}

fn call_hierarchy_item_for_symbol(
    relations: &RelationFacts<'_>,
    symbol: SymbolInfo,
) -> Option<CallHierarchyItem> {
    let kind = symbol.kind;
    if !is_call_hierarchy_kind(kind) {
        return None;
    }

    let full_range = symbol.definition_range?;
    let selection_range = symbol.selection_range.unwrap_or(full_range);
    let name = symbol.name.map(|name| name.to_string()).unwrap_or_else(|| "<anonymous>".to_owned());
    let detail = symbol
        .container
        .and_then(|container| relations.symbol(container))
        .and_then(|container| container.name.map(|name| name.to_string()));
    Some(CallHierarchyItem {
        symbol: Some(symbol.id),
        name,
        kind,
        detail,
        full_range,
        selection_range,
    })
}

fn is_call_hierarchy_kind(kind: SymbolKind) -> bool {
    matches!(kind, SymbolKind::Module)
}

fn push_call_range(groups: &mut Vec<(SymbolId, Vec<FileRange>)>, item: SymbolId, range: FileRange) {
    if let Some((_, ranges)) = groups.iter_mut().find(|(existing, _)| *existing == item) {
        if !ranges.contains(&range) {
            ranges.push(range);
        }
        return;
    }

    groups.push((item, vec![range]));
}

fn call_hierarchy_item_key(item: &CallHierarchyItem) -> (String, FileRange, FileRange) {
    (item.name.clone(), item.full_range, item.selection_range)
}
