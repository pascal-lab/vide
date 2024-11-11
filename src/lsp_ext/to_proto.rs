use anyhow::Error;
use ide::{
    Cancellable, SymbolKind, document_highlight::DocumentHighlight, navigation_target::NavTarget,
    references::ReferenceCategory, rename::RenameError, source_change::SourceChange,
};
use itertools::Itertools;
use line_index::{TextRange, TextSize};
use span::FileRange;
use utils::{
    lines::{LineEnding, LineInfo, PositionEncoding},
    paths::{
        AbsPath,
        camino::{Utf8Component, Utf8Prefix},
    },
    text_edit::{TextEdit, TextEditItem},
};
use vfs::FileId;

use super::lsp_error::LspError;
use crate::global_state::snapshot::GlobalStateSnapshot;

pub(crate) fn goto_definition_response(
    snap: &GlobalStateSnapshot,
    src: Option<FileRange>,
    targets: Vec<NavTarget>,
) -> Cancellable<lsp_types::GotoDefinitionResponse> {
    let res = if snap.config.location_link() {
        let links = targets
            .into_iter()
            .unique_by(|NavTarget { file_id, full_range, focus_range, .. }| {
                (*file_id, *full_range, *focus_range)
            })
            .map(|nav| location_link(snap, src, nav))
            .collect::<Cancellable<Vec<_>>>()?;
        links.into()
    } else {
        let locations = targets
            .into_iter()
            .map(|nav| FileRange { file_id: nav.file_id, range: nav.focus_or_full_range() })
            .unique()
            .map(|file_range| location(snap, file_range))
            .collect::<Cancellable<Vec<_>>>()?;
        locations.into()
    };
    Ok(res)
}

pub(crate) fn document_symbol(
    line_info: &LineInfo,
    symbol: ide::document_symbols::DocumentSymbol,
) -> lsp_types::DocumentSymbol {
    let children = symbol
        .children
        .map(|it| it.into_iter().map(|child| document_symbol(line_info, child)).collect());

    lsp_types::DocumentSymbol {
        name: symbol.name,
        detail: symbol.detail,
        kind: symbol_kind(symbol.kind),
        tags: None,
        deprecated: None,
        range: self::range(line_info, symbol.full_range),
        selection_range: self::range(line_info, symbol.focus_range),
        children,
    }
}

pub(crate) fn document_symbol_information(
    symbol: ide::document_symbols::DocumentSymbol,
    url: lsp_types::Url,
    line_info: &LineInfo,
    res: &mut Vec<lsp_types::SymbolInformation>,
) {
    res.push(lsp_types::SymbolInformation {
        name: symbol.name,
        kind: symbol_kind(symbol.kind),
        tags: None,
        deprecated: None,
        location: lsp_types::Location::new(url.clone(), self::range(line_info, symbol.focus_range)),
        container_name: symbol.container_name,
    });

    if let Some(children) = symbol.children {
        for child in children {
            document_symbol_information(child, url.clone(), line_info, res);
        }
    }
}

pub(crate) fn document_highlight(
    line_info: &LineInfo,
    DocumentHighlight { range, category }: DocumentHighlight,
) -> lsp_types::DocumentHighlight {
    let kind = if category.contains(ReferenceCategory::READ) {
        Some(lsp_types::DocumentHighlightKind::READ)
    } else if category.contains(ReferenceCategory::WRITE) {
        Some(lsp_types::DocumentHighlightKind::WRITE)
    } else {
        None
    };

    lsp_types::DocumentHighlight { range: self::range(line_info, range), kind }
}

fn symbol_kind(symbol_kind: SymbolKind) -> lsp_types::SymbolKind {
    use lsp_types::SymbolKind as LspSymbolKind;
    match symbol_kind {
        SymbolKind::Module => LspSymbolKind::MODULE,
        SymbolKind::PortLabel => LspSymbolKind::FIELD,
        SymbolKind::Decl => LspSymbolKind::VARIABLE,
        SymbolKind::Instance => LspSymbolKind::OBJECT,
        SymbolKind::Block => LspSymbolKind::NAMESPACE,
        SymbolKind::Stmt => LspSymbolKind::NAMESPACE,
        SymbolKind::Fn => LspSymbolKind::FUNCTION,
        SymbolKind::Generate => LspSymbolKind::NAMESPACE,
        SymbolKind::Interface => LspSymbolKind::INTERFACE,
    }
}

fn location_link(
    snap: &GlobalStateSnapshot,
    src: Option<FileRange>,
    target: NavTarget,
) -> Cancellable<lsp_types::LocationLink> {
    let origin_selection_range = try {
        let FileRange { file_id, range } = src?;
        let line_info = snap.line_info(file_id).ok()?;
        self::range(&line_info, range)
    };

    let (target_uri, target_range, target_selection_range) = location_info(snap, target)?;
    let res = lsp_types::LocationLink {
        origin_selection_range,
        target_uri,
        target_range,
        target_selection_range,
    };
    Ok(res)
}

fn location_info(
    snap: &GlobalStateSnapshot,
    NavTarget { file_id, full_range, focus_range, .. }: NavTarget,
) -> Cancellable<(lsp_types::Url, lsp_types::Range, lsp_types::Range)> {
    let line_info = snap.line_info(file_id)?;

    let target_uri = url(snap, file_id);
    let target_range = self::range(&line_info, full_range);
    let target_selection_range =
        focus_range.map(|it| self::range(&line_info, it)).unwrap_or(target_range);
    Ok((target_uri, target_range, target_selection_range))
}

pub(crate) fn url(snap: &GlobalStateSnapshot, file_id: FileId) -> lsp_types::Url {
    snap.url(file_id)
}

// Returns a `Url` object from a given path, will lowercase drive letters if
// present.
//
// This will only happen when processing windows paths.
// When processing non-windows path, this is the same as `Url::from_file_path`.
pub(crate) fn url_from_abs_path(path: &AbsPath) -> lsp_types::Url {
    let url = lsp_types::Url::from_file_path(path).unwrap();

    let Some(Utf8Component::Prefix(prefix)) = path.components().next() else {
        return url;
    };

    if !matches!(prefix.kind(), Utf8Prefix::Disk(_) | Utf8Prefix::VerbatimDisk(_)) {
        return url;
    }

    let Some((scheme, drive_letter, _)) = url.as_str().splitn(3, ':').collect_tuple() else {
        return url;
    };

    let start = scheme.len() + ':'.len_utf8();
    let driver_letter_range = start..(start + drive_letter.len());

    // lowercasing the `path` itself doesn't help, the `Url::parse` machinery
    // also canonicalizes the drive letter.
    let mut url: String = url.into();
    url[driver_letter_range].make_ascii_lowercase();
    lsp_types::Url::parse(&url).unwrap()
}

pub(crate) fn range(line_info: &LineInfo, range: TextRange) -> lsp_types::Range {
    let start = position(line_info, range.start());
    let end = position(line_info, range.end());
    lsp_types::Range::new(start, end)
}

pub(crate) fn location(
    snap: &GlobalStateSnapshot,
    FileRange { file_id, range }: FileRange,
) -> Cancellable<lsp_types::Location> {
    let url = url(snap, file_id);
    let line_info = snap.line_info(file_id)?;
    let range = self::range(&line_info, range);
    Ok(lsp_types::Location::new(url, range))
}

pub(crate) fn position(
    LineInfo { index, encoding, .. }: &LineInfo,
    offset: TextSize,
) -> lsp_types::Position {
    let line_col = index.line_col(offset);
    match *encoding {
        PositionEncoding::Utf8 => lsp_types::Position::new(line_col.line, line_col.col),
        PositionEncoding::Wide(enc) => {
            let line_col = index.to_wide(enc, line_col).unwrap();
            lsp_types::Position::new(line_col.line, line_col.col)
        }
    }
}

pub(crate) fn rename_error(err: RenameError) -> LspError {
    LspError::new(lsp_server::ErrorCode::InvalidParams as i32, err.to_string())
}

pub(crate) fn format_error(err: Error) -> LspError {
    LspError::new(lsp_server::ErrorCode::InternalError as i32, err.to_string())
}

pub(crate) fn workspace_edit(
    snap: &GlobalStateSnapshot,
    source_change: SourceChange,
) -> Cancellable<lsp_types::WorkspaceEdit> {
    let mut document_changes = Vec::new();

    for (file_id, edit) in source_change.text_edits {
        let text_document = optional_versioned_text_document_identifier(snap, file_id);
        let line_info = snap.line_info(file_id)?;
        let edits =
            edit.into_iter().map(|it| lsp_types::OneOf::Left(text_edit(&line_info, it))).collect();
        document_changes.push(lsp_types::TextDocumentEdit { text_document, edits });
    }

    let workspace_edit = lsp_types::WorkspaceEdit {
        changes: None,
        document_changes: Some(lsp_types::DocumentChanges::Edits(document_changes)),
        change_annotations: None,
    };

    Ok(workspace_edit)
}

pub(crate) fn optional_versioned_text_document_identifier(
    snap: &GlobalStateSnapshot,
    file_id: FileId,
) -> lsp_types::OptionalVersionedTextDocumentIdentifier {
    let url = url(snap, file_id);
    let version = snap.url_file_version(&url);
    lsp_types::OptionalVersionedTextDocumentIdentifier { uri: url, version }
}

pub(crate) fn text_edit(line_info: &LineInfo, item: TextEditItem) -> lsp_types::TextEdit {
    let range = self::range(line_info, item.del);
    let new_text = match line_info.ending {
        LineEnding::Unix => item.ins,
        LineEnding::Dos => item.ins.replace('\n', "\r\n"),
    };
    lsp_types::TextEdit { range, new_text }
}

pub(crate) fn text_edits(line_info: &LineInfo, edit: TextEdit) -> Vec<lsp_types::TextEdit> {
    edit.into_iter().map(|it| self::text_edit(line_info, it)).collect()
}
