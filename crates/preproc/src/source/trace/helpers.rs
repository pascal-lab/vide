use super::*;

pub(super) trait TraceTokenOptionExt {
    fn value(&self) -> Option<SmolStr>;
    fn macro_name(&self) -> Option<SmolStr>;
    fn source_range(&self) -> Option<SourceRange>;
    fn include_target(&self) -> MacroIncludeTarget;
}

impl TraceTokenOptionExt for Option<Token> {
    fn value(&self) -> Option<SmolStr> {
        self.as_ref().map(|token| token.value_text.to_smolstr())
    }

    fn macro_name(&self) -> Option<SmolStr> {
        self.as_ref().map(|token| macro_name(token.value_text.as_str()))
    }

    fn source_range(&self) -> Option<SourceRange> {
        self.as_ref().and_then(|token| trace_range(&token.range))
    }

    fn include_target(&self) -> MacroIncludeTarget {
        self.as_ref()
            .map(|token| include_target_from_raw(token.raw_text.to_smolstr()))
            .unwrap_or_else(|| MacroIncludeTarget::Token { raw: SmolStr::new("") })
    }
}

pub(super) fn trace_range(range: &Option<SourceBufferRange>) -> Option<SourceRange> {
    range.as_ref().and_then(source_range_from_trace)
}

pub(super) fn source_range_from_trace(range: &SourceBufferRange) -> Option<SourceRange> {
    Some(SourceRange {
        source: PreprocSourceId::from(range.buffer_id),
        range: TextRange::new(
            TextSize::from(u32::try_from(range.range.start).ok()?),
            TextSize::from(u32::try_from(range.range.end).ok()?),
        ),
    })
}

pub(super) fn event_kind(kind: SyntaxKind) -> Option<MacroEventKind> {
    match kind {
        SyntaxKind::DEFINE_DIRECTIVE => Some(MacroEventKind::Define),
        SyntaxKind::UNDEF_DIRECTIVE => Some(MacroEventKind::Undef),
        SyntaxKind::INCLUDE_DIRECTIVE => Some(MacroEventKind::Include),
        SyntaxKind::IF_DEF_DIRECTIVE
        | SyntaxKind::IF_N_DEF_DIRECTIVE
        | SyntaxKind::ELS_IF_DIRECTIVE => Some(MacroEventKind::Conditional),
        SyntaxKind::ELSE_DIRECTIVE | SyntaxKind::END_IF_DIRECTIVE => Some(MacroEventKind::Branch),
        SyntaxKind::MACRO_USAGE => Some(MacroEventKind::Usage),
        _ => None,
    }
}

pub(super) fn trace_conditional_kind(kind: SyntaxKind) -> MacroConditionalKind {
    match kind {
        SyntaxKind::IF_DEF_DIRECTIVE => MacroConditionalKind::IfDef,
        SyntaxKind::IF_N_DEF_DIRECTIVE => MacroConditionalKind::IfNDef,
        SyntaxKind::ELS_IF_DIRECTIVE => MacroConditionalKind::ElsIf,
        SyntaxKind::ELSE_DIRECTIVE => MacroConditionalKind::Else,
        SyntaxKind::END_IF_DIRECTIVE => MacroConditionalKind::EndIf,
        _ => unreachable!(),
    }
}

pub(super) fn push_source_event_record(
    index: &mut SourcePreprocIndex,
    event_id: SourcePreprocEventId,
    kind: MacroEventKind,
    event_index: usize,
    range: SourceRange,
) {
    index.event_records.push(SourcePreprocEventRecord {
        event_id,
        kind,
        range,
        index: event_index,
    });
}

fn include_target_from_raw(raw: SmolStr) -> MacroIncludeTarget {
    if let Some(path) = strip_include_delimiters(&raw) {
        MacroIncludeTarget::Literal { path: path.to_smolstr(), raw }
    } else {
        MacroIncludeTarget::Token { raw }
    }
}

fn strip_include_delimiters(raw: &str) -> Option<&str> {
    let bytes = raw.as_bytes();
    let (first, last) = (*bytes.first()?, *bytes.last()?);
    match (first, last) {
        (b'"', b'"') | (b'<', b'>') if raw.len() >= 2 => Some(&raw[1..raw.len() - 1]),
        _ => None,
    }
}

fn macro_name(name: &str) -> SmolStr {
    name.strip_prefix('`').unwrap_or(name).to_smolstr()
}
