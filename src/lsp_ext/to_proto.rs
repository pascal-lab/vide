use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::Error;
use hir::container::InFile;
use ide::{
    Cancellable, SymbolKind,
    code_action::{CodeAction, CodeActionKind},
    code_lens::{CodeLens, CodeLensKind},
    diagnostics as ide_diagnostics,
    document_highlight::DocumentHighlight,
    folding_ranges::{Fold, FoldingConfig},
    hover::HoverFormat,
    inlay_hint::{InlayHint, InlayKind},
    markup::Markup,
    navigation_target::NavTarget,
    references::ReferenceCategory,
    rename::RenameError,
    semantic_tokens::{SemaToken, SemaTokenModifier, SemaTokenPort, SemaTokenTag},
    signature_help::SignatureHelp,
    source_change::SourceChange,
};
use itertools::Itertools;
use span::{FilePosition, FileRange};
use syntax::DiagnosticSeverity as SlangDiagnosticSeverity;
use utils::{
    line_index::{TextRange, TextSize},
    lines::{LineEnding, LineInfo, PositionEncoding},
    paths::{
        AbsPath,
        camino::{Utf8Component, Utf8Prefix},
    },
    text_edit::{TextEdit, TextEditItem},
};
use vfs::FileId;

use super::{
    ext::{self, CodeLensData, CodeLensDataKind},
    lsp_error::LspError,
};
use crate::{
    global_state::snapshot::GlobalStateSnapshot,
    lsp_ext::ext::{
        SEMA_TOKENS_TYPES, SemaTokenModifierSet, sema_token_modifiers, sema_token_types,
    },
};

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

#[allow(deprecated)]
pub(crate) fn document_symbol(
    line_info: &LineInfo,
    symbol: ide::document_symbols::DocumentSymbol,
) -> lsp_types::DocumentSymbol {
    let children =
        symbol.children.into_iter().map(|child| document_symbol(line_info, child)).collect_vec();

    lsp_types::DocumentSymbol {
        name: symbol.name,
        detail: symbol.detail,
        kind: symbol_kind(symbol.kind),
        tags: None,
        deprecated: None,
        range: self::range(line_info, symbol.full_range),
        selection_range: self::range(line_info, symbol.focus_range),
        children: if children.is_empty() { None } else { Some(children) },
    }
}

#[allow(deprecated)]
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

    for child in symbol.children {
        document_symbol_information(child, url.clone(), line_info, res);
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

const SLANG_DIAGNOSTIC_SOURCE: &str = "slang";
const VIZSLA_DIAGNOSTIC_SOURCE: &str = "vizsla";

pub(crate) fn diagnostic(
    line_info: &LineInfo,
    diag: ide_diagnostics::Diagnostic,
) -> lsp_types::Diagnostic {
    let data = diagnostic_data(&diag);
    lsp_types::Diagnostic {
        range: self::range(line_info, diag.range),
        severity: diagnostic_severity(diag.severity),
        code: Some(lsp_types::NumberOrString::String(format!("{}:{}", diag.subsystem, diag.code))),
        code_description: None,
        source: Some(
            match diag.source {
                ide_diagnostics::DiagnosticSource::SlangParse
                | ide_diagnostics::DiagnosticSource::SlangSemantic => SLANG_DIAGNOSTIC_SOURCE,
                ide_diagnostics::DiagnosticSource::VizslaModel => VIZSLA_DIAGNOSTIC_SOURCE,
            }
            .to_string(),
        ),
        message: diag.message,
        related_information: None,
        tags: None,
        data: Some(data),
    }
}

fn diagnostic_data(diag: &ide_diagnostics::Diagnostic) -> serde_json::Value {
    serde_json::json!({
        "source": match diag.source {
            ide_diagnostics::DiagnosticSource::SlangParse => "parse",
            ide_diagnostics::DiagnosticSource::SlangSemantic => "semantic",
            ide_diagnostics::DiagnosticSource::VizslaModel => "model",
        },
        "subsystem": diag.subsystem,
        "code": diag.code,
        "name": diag.name,
        "option": diag.option_name,
        "groups": diag.groups,
        "selectorHints": diagnostic_selector_hints(diag),
    })
}

fn diagnostic_selector_hints(diag: &ide_diagnostics::Diagnostic) -> Vec<String> {
    let mut selectors = vec![format!("code:{}:{}", diag.subsystem, diag.code)];

    selectors.push(format!("name:{}", diag.name));

    if let Some(option) = &diag.option_name {
        selectors.push(format!("option:{option}"));
    }

    selectors.extend(diag.groups.iter().map(|group| format!("group:{group}")));
    selectors.push(match diag.source {
        ide_diagnostics::DiagnosticSource::SlangParse => "source:parse".to_owned(),
        ide_diagnostics::DiagnosticSource::SlangSemantic => "source:semantic".to_owned(),
        ide_diagnostics::DiagnosticSource::VizslaModel => "source:model".to_owned(),
    });

    selectors
}

fn diagnostic_severity(severity: SlangDiagnosticSeverity) -> Option<lsp_types::DiagnosticSeverity> {
    use lsp_types::DiagnosticSeverity as LspSeverity;
    match severity {
        SlangDiagnosticSeverity::Ignored => None,
        SlangDiagnosticSeverity::Note => Some(LspSeverity::INFORMATION),
        SlangDiagnosticSeverity::Warning => Some(LspSeverity::WARNING),
        SlangDiagnosticSeverity::Error | SlangDiagnosticSeverity::Fatal => Some(LspSeverity::ERROR),
    }
}

fn symbol_kind(symbol_kind: SymbolKind) -> lsp_types::SymbolKind {
    use lsp_types::SymbolKind as LspSymbolKind;
    match symbol_kind {
        SymbolKind::Module => LspSymbolKind::MODULE,
        SymbolKind::NonAnsiPortLabel => LspSymbolKind::FIELD,
        SymbolKind::PortDecl => LspSymbolKind::FIELD,
        SymbolKind::ParamDecl => LspSymbolKind::TYPE_PARAMETER,
        SymbolKind::NetDecl => LspSymbolKind::PROPERTY,
        SymbolKind::DataDecl => LspSymbolKind::VARIABLE,
        SymbolKind::Typedef => LspSymbolKind::TYPE_PARAMETER,
        SymbolKind::Instance => LspSymbolKind::OBJECT,
        SymbolKind::Block => LspSymbolKind::NAMESPACE,
        SymbolKind::Stmt => LspSymbolKind::NAMESPACE,
        SymbolKind::Fn => LspSymbolKind::FUNCTION,
        SymbolKind::Generate => LspSymbolKind::NAMESPACE,
        SymbolKind::Interface => LspSymbolKind::INTERFACE,
        SymbolKind::Region => LspSymbolKind::NAMESPACE,
        SymbolKind::Opaque => LspSymbolKind::NAMESPACE,
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
    let line_col = index.line_col(offset.min(index.text_len()));
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
    LspError::new(lsp_server::ErrorCode::RequestFailed as i32, err.to_string())
}

#[allow(dead_code)]
pub(crate) fn code_action_resolve_error(err: Error) -> LspError {
    LspError::new(lsp_server::ErrorCode::InvalidParams as i32, err.to_string())
}

pub(crate) fn workspace_edit(
    snap: &GlobalStateSnapshot,
    source_change: SourceChange,
) -> Cancellable<lsp_types::WorkspaceEdit> {
    let mut document_changes = Vec::with_capacity(source_change.text_edits.len());

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

pub(crate) fn selection_ranges(
    line_info: &LineInfo,
    ranges: Vec<TextRange>,
) -> lsp_types::SelectionRange {
    ranges
        .into_iter()
        .rfold(None, |parent, range| {
            Some(lsp_types::SelectionRange {
                range: self::range(line_info, range),
                parent: parent.map(Box::new),
            })
        })
        .unwrap()
}

pub(crate) fn folding_range(
    text: &str,
    line_info: &LineInfo,
    FoldingConfig { line_fold_only }: &FoldingConfig,
    Fold { range, kind, collapsed_text }: Fold,
) -> lsp_types::FoldingRange {
    use ide::folding_ranges::FoldKind;
    let kind = match kind {
        FoldKind::Comment => Some(lsp_types::FoldingRangeKind::Comment),
        FoldKind::Imports => Some(lsp_types::FoldingRangeKind::Imports),
        FoldKind::Region => Some(lsp_types::FoldingRangeKind::Region),
        _ => None,
    };

    let lsp_types::Range { start, end } = self::range(line_info, range);

    if *line_fold_only {
        // Clients with `line_folding_only` will fold the whole end line even if
        // it contains text not in the folding range. So we should exclude the end
        // line if there is more text after the end character on the same line.
        let range_end = range.end().into();
        let end_line = if range_end < text.len() && has_more_text_in_line(&text[range_end..]) {
            end.line
        } else {
            end.line.saturating_sub(1)
        };

        lsp_types::FoldingRange {
            start_line: start.line,
            start_character: None,
            end_line,
            end_character: None,
            kind,
            collapsed_text,
        }
    } else {
        lsp_types::FoldingRange {
            start_line: start.line,
            start_character: Some(start.character),
            end_line: end.line,
            end_character: Some(end.character),
            kind,
            collapsed_text,
        }
    }
}

fn has_more_text_in_line(text: &str) -> bool {
    let mut iter = text.chars().peekable();

    let mut met_first_punct = false;
    while let Some(c) = iter.next() {
        match c {
            ',' | ';' => {
                if met_first_punct {
                    return false;
                } else {
                    met_first_punct = true;
                }
            }
            '\n' => return true,
            '/' if iter.next_if_eq(&'/').is_some() => return true,
            '/' if iter.next_if_eq(&'*').is_some() => {
                while let Some(c) = iter.next() {
                    if c == '*' && iter.next_if_eq(&'/').is_some() {
                        break;
                    } else if c == '\n' {
                        return false;
                    }
                }
            }
            _ if c.is_whitespace() => {}
            _ => return false,
        }
    }

    true
}

pub(crate) fn hover_contents(markup: Markup, format: HoverFormat) -> lsp_types::HoverContents {
    let kind = match format {
        HoverFormat::Markdown => lsp_types::MarkupKind::Markdown,
        HoverFormat::PlainText => lsp_types::MarkupKind::PlainText,
    };

    let value = markup.into();
    lsp_types::HoverContents::Markup(lsp_types::MarkupContent { kind, value })
}

pub(crate) fn inlay_hint(
    snap: &GlobalStateSnapshot,
    line_info: &LineInfo,
    hint: InlayHint,
) -> lsp_types::InlayHint {
    let InlayHint {
        label,
        tooltip,
        target_location,
        position,
        kind,
        text_edit,
        padding_left,
        padding_right,
    } = hint;

    let label = lsp_types::InlayHintLabelPart {
        value: label,
        tooltip: tooltip.map(|tooltip| {
            lsp_types::InlayHintLabelPartTooltip::MarkupContent(lsp_types::MarkupContent {
                kind: lsp_types::MarkupKind::Markdown,
                value: tooltip.into(),
            })
        }),
        location: target_location.and_then(|InFile { value, file_id }| {
            let file_range = FileRange { file_id: file_id.file_id(), range: value };
            self::location(snap, file_range).ok()
        }),
        command: None,
    };

    let position = self::position(line_info, position);
    let kind = match kind {
        InlayKind::ParamAssign | InlayKind::Port => Some(lsp_types::InlayHintKind::PARAMETER),
        InlayKind::EndStructure => None,
    };

    let text_edits = text_edit.map(|it| self::text_edits(line_info, it));

    lsp_types::InlayHint {
        position,
        label: lsp_types::InlayHintLabel::LabelParts(vec![label]),
        kind,
        text_edits,
        tooltip: None,
        padding_left: Some(padding_left),
        padding_right: Some(padding_right),
        data: None,
    }
}

pub(crate) fn code_lens(
    snap: &GlobalStateSnapshot,
    line_info: &LineInfo,
    file_id: FileId,
    CodeLens { range, kind }: CodeLens,
) -> Option<lsp_types::CodeLens> {
    let range = self::range(line_info, range);
    let (command, data) = self::code_lens_kind(snap, file_id, line_info, kind).ok()?;
    Some(lsp_types::CodeLens { range, command, data })
}

pub(crate) fn code_lens_kind(
    snap: &GlobalStateSnapshot,
    file_id: FileId,
    line_info: &LineInfo,
    kind: CodeLensKind,
) -> anyhow::Result<(Option<lsp_types::Command>, Option<serde_json::Value>)> {
    let url = self::url(snap, file_id);

    let command = match kind {
        CodeLensKind::ModuleInstance { data, .. } => data.map(|ranges| {
            let count = ranges.len();
            let s = if count == 1 { "" } else { "s" };
            lsp_types::Command {
                title: format!("{count} instance{s}"),
                command: String::new(),
                arguments: None,
            }
        }),
    };

    let data = match kind {
        CodeLensKind::ModuleInstance { pos: FilePosition { offset, .. }, .. } => {
            snap.url_file_version(&url).and_then(|version| {
                let file_pos = lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: self::url(snap, file_id),
                    },
                    position: self::position(line_info, offset),
                };
                let data =
                    CodeLensData { version, kind: CodeLensDataKind::Instantiation(file_pos) };
                serde_json::to_value(data).ok()
            })
        }
    };

    Ok((command, data))
}

pub(crate) struct SemanticTokensBuilder {
    id: String,
    prev_line: u32,
    prev_char: u32,
    data: Vec<lsp_types::SemanticToken>,
}

impl SemanticTokensBuilder {
    pub(crate) fn new(id: String) -> Self {
        SemanticTokensBuilder { id, prev_line: 0, prev_char: 0, data: Default::default() }
    }

    /// Push a new token onto the builder
    pub(crate) fn push(
        &mut self,
        range: lsp_types::Range,
        token_type: u32,
        token_modifiers_bitset: u32,
    ) {
        let lsp_types::Position { line: mut push_line, character: mut push_char } = range.start;

        if !self.data.is_empty() {
            push_line -= self.prev_line;
            if push_line == 0 {
                push_char -= self.prev_char;
            }
        }

        let token = lsp_types::SemanticToken {
            delta_line: push_line,
            delta_start: push_char,
            length: range.end.character - range.start.character, // Token can be multiline
            token_type,
            token_modifiers_bitset,
        };

        self.data.push(token);

        self.prev_line = range.start.line;
        self.prev_char = range.start.character;
    }

    pub(crate) fn build(self) -> lsp_types::SemanticTokens {
        lsp_types::SemanticTokens { result_id: Some(self.id), data: self.data }
    }
}

pub(crate) fn semantic_tokens(
    text: &str,
    line_info: &LineInfo,
    sema_tokens: Vec<SemaToken>,
) -> lsp_types::SemanticTokens {
    static TOKEN_RESULT_COUNTER: AtomicU32 = AtomicU32::new(1);

    let id = TOKEN_RESULT_COUNTER.fetch_add(1, Ordering::SeqCst).to_string();
    let mut builder = SemanticTokensBuilder::new(id);

    for SemaToken { range, tag, mods } in sema_tokens.into_iter().filter(|tok| !tok.is_empty()) {
        let ty = match tag {
            SemaTokenTag::Port(SemaTokenPort::Clk) => sema_token_types::CLK_PORT,
            SemaTokenTag::Port(SemaTokenPort::Rst) => sema_token_types::RST_PORT,
            SemaTokenTag::Port(SemaTokenPort::Others) => sema_token_types::OTHERS_PORT,
            SemaTokenTag::Instance => sema_token_types::INSTANCE,
            SemaTokenTag::Type => sema_token_types::TYPE_ALIAS,
            SemaTokenTag::None => sema_token_types::GENERIC,
        };
        // WORKAROUND: currently we haven't implemented client.
        let ty = sema_token_types::fallback(ty).unwrap();
        let ty = SEMA_TOKENS_TYPES.iter().position(|it| it == &ty).unwrap() as u32;

        let mut mods_set = SemaTokenModifierSet::default();
        for modifier in mods {
            let modifier = match modifier {
                SemaTokenModifier::DECL => sema_token_modifiers::DECLARATION,
                SemaTokenModifier::DEF => sema_token_modifiers::DEF,
                SemaTokenModifier::READ => sema_token_modifiers::READ,
                SemaTokenModifier::WRITE => sema_token_modifiers::WRITE,
                SemaTokenModifier::REF => sema_token_modifiers::REF,
                _ => unreachable!(),
            };
            // WORKAROUND: currently we haven't implemented client.
            mods_set |= sema_token_modifiers::fallback(modifier).unwrap();
        }
        let mods = mods_set.finish();

        for mut range in line_info.index.lines(range) {
            if text[range].ends_with('\n') {
                range = TextRange::new(range.start(), range.end() - TextSize::of('\n'));
            }
            let range = self::range(line_info, range);
            builder.push(range, ty, mods);
        }
    }

    builder.build()
}

pub(crate) fn semantic_token_delta(
    lsp_types::SemanticTokens { data: old, .. }: &lsp_types::SemanticTokens,
    lsp_types::SemanticTokens { data: new, result_id }: &lsp_types::SemanticTokens,
) -> lsp_types::SemanticTokensDelta {
    let old = old.as_slice();
    let new = new.as_slice();

    let offset = new.iter().zip(old.iter()).take_while(|&(n, p)| n == p).count();
    let (_, old) = old.split_at(offset);
    let (_, new) = new.split_at(offset);

    let offset_from_end =
        new.iter().rev().zip(old.iter().rev()).take_while(|&(n, p)| n == p).count();

    let (old, _) = old.split_at(old.len() - offset_from_end);
    let (new, _) = new.split_at(new.len() - offset_from_end);

    let edits = if old.is_empty() && new.is_empty() {
        vec![]
    } else {
        const TOKEN_SIZE: u32 = 5;
        vec![lsp_types::SemanticTokensEdit {
            start: TOKEN_SIZE * (offset as u32),
            delete_count: TOKEN_SIZE * (old.len() as u32),
            data: Some(new.into()),
        }]
    };

    lsp_types::SemanticTokensDelta { result_id: result_id.clone().clone(), edits }
}

pub(crate) fn signature_help(
    sig_help: SignatureHelp,
    support_label_offsets: bool,
) -> lsp_types::SignatureHelp {
    let parameters = if support_label_offsets {
        sig_help
            .param_ranges
            .iter()
            .map(|it| {
                let start = sig_help.label[..it.start().into()].chars().count() as u32;
                let end = sig_help.label[..it.end().into()].chars().count() as u32;
                [start, end]
            })
            .map(|range| lsp_types::ParameterInformation {
                label: lsp_types::ParameterLabel::LabelOffsets(range),
                documentation: None,
            })
            .collect()
    } else {
        sig_help
            .param_ranges
            .iter()
            .map(|range| lsp_types::ParameterInformation {
                label: lsp_types::ParameterLabel::Simple(sig_help.label[*range].to_owned()),
                documentation: None,
            })
            .collect()
    };

    let active_parameter = sig_help.active_parameter.map(|it| it as u32);

    let signature = lsp_types::SignatureInformation {
        label: sig_help.label,
        documentation: None,
        parameters: Some(parameters),
        active_parameter,
    };

    lsp_types::SignatureHelp {
        signatures: vec![signature],
        active_signature: Some(0),
        active_parameter,
    }
}

pub(crate) fn code_action_kind(kind: CodeActionKind) -> lsp_types::CodeActionKind {
    match kind {
        CodeActionKind::Generate => lsp_types::CodeActionKind::EMPTY,
        CodeActionKind::QuickFix => lsp_types::CodeActionKind::QUICKFIX,
        CodeActionKind::Refactor => lsp_types::CodeActionKind::REFACTOR,
        CodeActionKind::RefactorExtract => lsp_types::CodeActionKind::REFACTOR_EXTRACT,
        CodeActionKind::RefactorInline => lsp_types::CodeActionKind::REFACTOR_INLINE,
        CodeActionKind::RefactorRewrite => lsp_types::CodeActionKind::REFACTOR_REWRITE,
    }
}

pub(crate) fn code_action(
    snap: &GlobalStateSnapshot,
    CodeAction { id, label, source_change, .. }: CodeAction,
    resolve_data: Option<(usize, lsp_types::CodeActionParams, Option<i32>)>,
    diagnostics: Option<Vec<lsp_types::Diagnostic>>,
) -> Cancellable<lsp_types::CodeAction> {
    let mut res = lsp_types::CodeAction {
        title: label,
        kind: Some(self::code_action_kind(id.kind)),
        edit: None,
        is_preferred: None,
        command: None, // TODO: fill commands
        diagnostics,
        disabled: None,
        data: None,
    };

    match (source_change, resolve_data) {
        (Some(it), _) => res.edit = Some(workspace_edit(snap, it)?),
        (None, Some((idx, code_action_params, version))) => {
            let data = ext::CodeActionData {
                id: format!("{}:{idx}", id.name),
                code_action_params,
                version,
            };
            res.data = Some(serde_json::to_value(data).unwrap());
        }
        (None, None) => {
            unreachable!("code actions should be resolved if client can't resolve lazily")
        }
    };
    Ok(res)
}
