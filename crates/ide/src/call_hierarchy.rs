use itertools::Itertools;
use utils::text_edit::{TextRange, TextSize};

use crate::{
    FilePosition, FileRange, SymbolKind,
    db::root_db::RootDb,
    document_symbols::DocumentSymbol,
    facts::{SemanticFacts, relation::RelationFacts},
    navigation_target::NavTarget,
    references::ReferencesConfig,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallHierarchyItem {
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
    let nav_info = facts.relations().definition_targets(position)?;
    let items = nav_info
        .info
        .into_iter()
        .filter_map(call_hierarchy_item_for_nav)
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
    let mut groups = Vec::<(CallHierarchyItem, Vec<FileRange>)>::new();
    for reference in reference_ranges_for_call_item(&relations, &target, config.clone()) {
        if reference == target.selection_range {
            continue;
        }

        let Some(caller) = enclosing_module_item(&relations, reference) else {
            continue;
        };
        push_call_range(&mut groups, caller, reference);
    }

    let calls: Vec<_> =
        groups.into_iter().map(|(from, from_ranges)| IncomingCall { from, from_ranges }).collect();
    (!calls.is_empty()).then_some(calls)
}

pub(crate) fn outgoing(
    db: &RootDb,
    caller: CallHierarchyItem,
    config: ReferencesConfig,
) -> Option<Vec<OutgoingCall>> {
    let facts = SemanticFacts::new(db);
    let relations = facts.relations();
    let mut groups = Vec::<(CallHierarchyItem, Vec<FileRange>)>::new();
    for callee in workspace_module_items(&relations) {
        if same_call_hierarchy_item(&caller, &callee) {
            continue;
        }

        for reference in reference_ranges_for_call_item(&relations, &callee, config.clone()) {
            if reference.file_id == caller.full_range.file_id
                && range_contains_range(caller.full_range.range, reference.range)
            {
                push_call_range(&mut groups, callee.clone(), reference);
            }
        }
    }

    let calls: Vec<_> =
        groups.into_iter().map(|(to, from_ranges)| OutgoingCall { to, from_ranges }).collect();
    (!calls.is_empty()).then_some(calls)
}

fn reference_ranges_for_call_item(
    relations: &RelationFacts<'_>,
    item: &CallHierarchyItem,
    config: ReferencesConfig,
) -> Vec<FileRange> {
    let position = FilePosition {
        file_id: item.selection_range.file_id,
        offset: item.selection_range.range.start(),
    };
    relations.reference_ranges(position, config)
}

fn call_hierarchy_item_for_nav(nav: NavTarget) -> Option<CallHierarchyItem> {
    let kind = nav.kind?;
    if !is_call_hierarchy_kind(kind) {
        return None;
    }

    let selection_range = nav.focus_or_full_range();
    let name = nav
        .name
        .map(|name| name.to_string())
        .unwrap_or_else(|| nav.description.clone().unwrap_or_else(|| "<anonymous>".to_owned()));
    let detail = nav.container_name.map(|name| name.to_string()).or(nav.description);
    Some(CallHierarchyItem {
        name,
        kind,
        detail,
        full_range: FileRange { file_id: nav.file_id, range: nav.full_range },
        selection_range: FileRange { file_id: nav.file_id, range: selection_range },
    })
}

fn workspace_module_items(relations: &RelationFacts<'_>) -> Vec<CallHierarchyItem> {
    let mut file_ids = relations.file_ids();
    file_ids.sort_unstable_by_key(|file_id| file_id.0);
    file_ids.dedup();

    let mut items = Vec::new();
    for file_id in file_ids {
        for symbol in relations.document_symbols(file_id) {
            collect_module_items(file_id, symbol, &mut items);
        }
    }

    items.into_iter().unique_by(call_hierarchy_item_key).collect()
}

fn collect_module_items(
    file_id: vfs::FileId,
    symbol: DocumentSymbol,
    items: &mut Vec<CallHierarchyItem>,
) {
    if symbol.kind == SymbolKind::Module {
        items.push(call_hierarchy_item_for_symbol(file_id, &symbol));
    }

    for child in symbol.children {
        collect_module_items(file_id, child, items);
    }
}

fn enclosing_module_item(
    relations: &RelationFacts<'_>,
    range: FileRange,
) -> Option<CallHierarchyItem> {
    let mut best = None;
    for symbol in relations.document_symbols(range.file_id) {
        find_enclosing_module_symbol(symbol, range.range.start(), &mut best);
    }

    best.map(|symbol| call_hierarchy_item_for_symbol(range.file_id, &symbol))
}

fn find_enclosing_module_symbol(
    symbol: DocumentSymbol,
    offset: TextSize,
    best: &mut Option<DocumentSymbol>,
) {
    if !range_contains_offset(symbol.full_range, offset) {
        return;
    }

    if symbol.kind == SymbolKind::Module
        && best.as_ref().is_none_or(|current| symbol.full_range.len() < current.full_range.len())
    {
        *best = Some(symbol.clone());
    }

    for child in symbol.children {
        find_enclosing_module_symbol(child, offset, best);
    }
}

fn call_hierarchy_item_for_symbol(
    file_id: vfs::FileId,
    symbol: &DocumentSymbol,
) -> CallHierarchyItem {
    CallHierarchyItem {
        name: symbol.name.clone(),
        kind: symbol.kind,
        detail: symbol.container_name.clone(),
        full_range: FileRange { file_id, range: symbol.full_range },
        selection_range: FileRange { file_id, range: symbol.focus_range },
    }
}

fn is_call_hierarchy_kind(kind: SymbolKind) -> bool {
    matches!(kind, SymbolKind::Module)
}

fn push_call_range(
    groups: &mut Vec<(CallHierarchyItem, Vec<FileRange>)>,
    item: CallHierarchyItem,
    range: FileRange,
) {
    if let Some((_, ranges)) =
        groups.iter_mut().find(|(existing, _)| same_call_hierarchy_item(existing, &item))
    {
        if !ranges.contains(&range) {
            ranges.push(range);
        }
        return;
    }

    groups.push((item, vec![range]));
}

fn same_call_hierarchy_item(lhs: &CallHierarchyItem, rhs: &CallHierarchyItem) -> bool {
    lhs.name == rhs.name
        && lhs.full_range == rhs.full_range
        && lhs.selection_range == rhs.selection_range
}

fn call_hierarchy_item_key(item: &CallHierarchyItem) -> (String, FileRange, FileRange) {
    (item.name.clone(), item.full_range, item.selection_range)
}

fn range_contains_offset(range: TextRange, offset: TextSize) -> bool {
    range.start() <= offset && offset < range.end()
}

fn range_contains_range(container: TextRange, range: TextRange) -> bool {
    container.start() <= range.start() && range.end() <= container.end()
}
