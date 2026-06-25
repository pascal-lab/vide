use ide::{
    FileRange, SymbolKind,
    call_hierarchy::{CallHierarchyItem, IncomingCall, OutgoingCall},
    references::References,
};
use itertools::Itertools;

use crate::{
    global_state::snapshot::GlobalStateSnapshot,
    lsp_ext::{from_proto, to_proto},
};

pub(crate) fn handle_goto_definition(
    snap: GlobalStateSnapshot,
    params: lsp_types::GotoDefinitionParams,
) -> anyhow::Result<Option<lsp_types::GotoDefinitionResponse>> {
    let position = from_proto::file_position(&snap, params.text_document_position_params)?;
    let Some(nav_info) = snap.analysis.goto_definition(position)? else {
        return Ok(None);
    };

    let src = FileRange { file_id: position.file_id, range: nav_info.range };
    let res = to_proto::goto_definition_response(&snap, Some(src), nav_info.info)?;
    Ok(Some(res))
}

pub(crate) fn handle_goto_declaration(
    snap: GlobalStateSnapshot,
    params: lsp_types::request::GotoDeclarationParams,
) -> anyhow::Result<Option<lsp_types::request::GotoDeclarationResponse>> {
    let position = from_proto::file_position(&snap, params.text_document_position_params.clone())?;
    let Some(nav_info) = snap.analysis.goto_declaration(position)? else {
        return handle_goto_definition(snap, params);
    };
    let src = FileRange { file_id: position.file_id, range: nav_info.range };
    let res = to_proto::goto_definition_response(&snap, Some(src), nav_info.info)?;
    Ok(Some(res))
}

pub(crate) fn handle_goto_type_definition(
    snap: GlobalStateSnapshot,
    params: lsp_types::request::GotoTypeDefinitionParams,
) -> anyhow::Result<Option<lsp_types::request::GotoTypeDefinitionResponse>> {
    handle_goto_definition(snap, params)
}

pub(crate) fn handle_prepare_call_hierarchy(
    snap: GlobalStateSnapshot,
    params: lsp_types::CallHierarchyPrepareParams,
) -> anyhow::Result<Option<Vec<lsp_types::CallHierarchyItem>>> {
    let position = from_proto::file_position(&snap, params.text_document_position_params)?;
    let Some(items) = snap.analysis.prepare_call_hierarchy(position)? else {
        return Ok(None);
    };

    let items = items
        .into_iter()
        .map(|item| lsp_call_hierarchy_item(&snap, item))
        .collect::<anyhow::Result<Vec<_>>>()?
        .into_iter()
        .unique_by(lsp_call_hierarchy_item_key)
        .collect_vec();

    Ok((!items.is_empty()).then_some(items))
}

pub(crate) fn handle_call_hierarchy_incoming(
    snap: GlobalStateSnapshot,
    params: lsp_types::CallHierarchyIncomingCallsParams,
) -> anyhow::Result<Option<Vec<lsp_types::CallHierarchyIncomingCall>>> {
    let target = call_hierarchy_item_from_lsp(&snap, params.item)?;
    let config = snap.config.references();
    let Some(calls) = snap.analysis.call_hierarchy_incoming(target, config)? else {
        return Ok(None);
    };
    let calls = calls
        .into_iter()
        .map(|call| lsp_incoming_call(&snap, call))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok((!calls.is_empty()).then_some(calls))
}

pub(crate) fn handle_call_hierarchy_outgoing(
    snap: GlobalStateSnapshot,
    params: lsp_types::CallHierarchyOutgoingCallsParams,
) -> anyhow::Result<Option<Vec<lsp_types::CallHierarchyOutgoingCall>>> {
    let caller = call_hierarchy_item_from_lsp(&snap, params.item)?;
    let config = snap.config.references();
    let Some(calls) = snap.analysis.call_hierarchy_outgoing(caller, config)? else {
        return Ok(None);
    };
    let calls = calls
        .into_iter()
        .map(|call| lsp_outgoing_call(&snap, call))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok((!calls.is_empty()).then_some(calls))
}

pub(crate) fn handle_document_highlight(
    snap: GlobalStateSnapshot,
    params: lsp_types::DocumentHighlightParams,
) -> anyhow::Result<Option<Vec<lsp_types::DocumentHighlight>>> {
    let position = from_proto::file_position(&snap, params.text_document_position_params)?;
    let line_info = snap.line_info(position.file_id)?;
    let config = snap.config.document_highlight();
    let Some(highlights) = snap.analysis.document_highlight(position, config)? else {
        return Ok(None);
    };

    let res = highlights
        .into_iter()
        .map(|highlight| to_proto::document_highlight(&line_info, highlight))
        .collect();
    Ok(Some(res))
}

pub(crate) fn handle_references(
    snap: GlobalStateSnapshot,
    params: lsp_types::ReferenceParams,
) -> anyhow::Result<Option<Vec<lsp_types::Location>>> {
    let include_declaration =
        params.context.include_declaration && snap.config.references_include_declaration();
    let position = from_proto::file_position(&snap, params.text_document_position)?;
    let config = snap.config.references();
    let Some(refs) = snap.analysis.references(position, config)? else {
        return Ok(None);
    };
    let partial_issue_count: usize =
        refs.iter().map(|references| references.status.issue_count()).sum();
    if partial_issue_count > 0 {
        tracing::debug!(partial_issue_count, "references result is partial");
    }

    let locations = refs
        .into_iter()
        .flat_map(|References { def, refs, .. }| {
            let decl = if include_declaration { def.unwrap_or_default() } else { Vec::new() }
                .into_iter()
                .map(|nav| FileRange { file_id: nav.file_id, range: nav.focus_or_full_range() });

            let refs = refs.into_iter().flat_map(|(file_id, refs)| {
                refs.into_iter().map(move |(range, _)| FileRange { file_id, range })
            });

            decl.chain(refs)
        })
        .unique()
        .filter_map(|frange| to_proto::location(&snap, frange).ok())
        .collect_vec();

    Ok(Some(locations))
}

fn lsp_incoming_call(
    snap: &GlobalStateSnapshot,
    IncomingCall { from, from_ranges }: IncomingCall,
) -> anyhow::Result<lsp_types::CallHierarchyIncomingCall> {
    Ok(lsp_types::CallHierarchyIncomingCall {
        from: lsp_call_hierarchy_item(snap, from)?,
        from_ranges: lsp_call_ranges(snap, from_ranges)?,
    })
}

fn lsp_outgoing_call(
    snap: &GlobalStateSnapshot,
    OutgoingCall { to, from_ranges }: OutgoingCall,
) -> anyhow::Result<lsp_types::CallHierarchyOutgoingCall> {
    Ok(lsp_types::CallHierarchyOutgoingCall {
        to: lsp_call_hierarchy_item(snap, to)?,
        from_ranges: lsp_call_ranges(snap, from_ranges)?,
    })
}

fn lsp_call_hierarchy_item(
    snap: &GlobalStateSnapshot,
    item: CallHierarchyItem,
) -> anyhow::Result<lsp_types::CallHierarchyItem> {
    let line_info = snap.line_info(item.full_range.file_id)?;
    let uri = to_proto::url(snap, item.full_range.file_id)?;
    Ok(lsp_types::CallHierarchyItem {
        name: item.name,
        kind: to_proto::symbol_kind(item.kind),
        tags: None,
        detail: item.detail,
        uri,
        range: to_proto::range(&line_info, item.full_range.range),
        selection_range: to_proto::range(&line_info, item.selection_range.range),
        data: None,
    })
}

fn call_hierarchy_item_from_lsp(
    snap: &GlobalStateSnapshot,
    item: lsp_types::CallHierarchyItem,
) -> anyhow::Result<CallHierarchyItem> {
    let file_id = from_proto::file_id(snap, &item.uri)?;
    let line_info = snap.line_info(file_id)?;
    Ok(CallHierarchyItem {
        symbol: None,
        name: item.name,
        kind: symbol_kind_from_lsp(item.kind),
        detail: item.detail,
        full_range: FileRange { file_id, range: from_proto::text_range(&line_info, item.range)? },
        selection_range: FileRange {
            file_id,
            range: from_proto::text_range(&line_info, item.selection_range)?,
        },
    })
}

fn symbol_kind_from_lsp(kind: lsp_types::SymbolKind) -> SymbolKind {
    match kind {
        lsp_types::SymbolKind::MODULE => SymbolKind::Module,
        _ => SymbolKind::Unknown,
    }
}

fn lsp_call_ranges(
    snap: &GlobalStateSnapshot,
    ranges: Vec<FileRange>,
) -> anyhow::Result<Vec<lsp_types::Range>> {
    ranges
        .into_iter()
        .map(|range| {
            let line_info = snap.line_info(range.file_id)?;
            Ok(to_proto::range(&line_info, range.range))
        })
        .collect()
}

fn lsp_call_hierarchy_item_key(
    item: &lsp_types::CallHierarchyItem,
) -> (String, lsp_types::Url, lsp_types::Range, lsp_types::Range) {
    (item.name.clone(), item.uri.clone(), item.range, item.selection_range)
}

pub(crate) fn handle_hover(
    snap: GlobalStateSnapshot,
    params: lsp_types::HoverParams,
) -> anyhow::Result<Option<lsp_types::Hover>> {
    let position = from_proto::file_position(&snap, params.text_document_position_params)?;

    let config = snap.config.hover();
    let hover_format = config.format;
    let Some(hover_info) = snap.analysis.hover(position, config)? else {
        return Ok(None);
    };

    let line_info = snap.line_info(position.file_id)?;
    let range = to_proto::range(&line_info, hover_info.range);

    let res = lsp_types::Hover {
        contents: to_proto::hover_contents(hover_info.info, hover_format),
        range: Some(range),
    };

    Ok(Some(res))
}
