use hir::{
    db::HirDb,
    hir_def::macro_file::{MacroFileExpansion, macro_file_expansion, macro_files_at_offset},
    preproc::{MacroReferenceDefinitions, macro_reference_definitions_at},
};
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use super::markup::{
    render_macro_expansion_header, render_macro_expansion_separator,
    render_macro_expansion_source_link,
};
use crate::{RangeInfo, db::root_db::RootDb, markup::Markup};

pub(in crate::hover) fn with_expanded_macro_hover(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
    mut hover: RangeInfo<Markup>,
) -> RangeInfo<Markup> {
    let Some(expanded) = expanded_macro_hover(db, file_id, offset, None) else {
        return hover;
    };
    if let Some(range) = covering_range(&[hover.range, expanded.range]) {
        hover.range = range;
    }
    hover.info.horizontal_line();
    hover.info.merge(expanded.info);
    hover
}

pub(super) fn expanded_macro_hover(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
    reference_definitions: Option<&MacroReferenceDefinitions>,
) -> Option<RangeInfo<Markup>> {
    let reference_ranges = if let Some(reference_definitions) = reference_definitions {
        reference_definitions
            .references
            .iter()
            .map(|reference| (reference.file_id, reference.range))
            .collect::<Vec<_>>()
    } else {
        macro_reference_definitions_at(db, file_id, offset)
            .ok()
            .flatten()?
            .references
            .into_iter()
            .map(|reference| (reference.file_id, reference.range))
            .collect::<Vec<_>>()
    };
    if reference_ranges.is_empty() {
        return None;
    }

    let macro_files = macro_files_at_offset(db, file_id, offset);
    let expansions = macro_files
        .into_iter()
        .filter_map(|macro_file| {
            let metadata = macro_file_expansion(db, macro_file)?;
            if !reference_ranges.iter().any(|&(file_id, range)| {
                file_id == metadata.call_file_id
                    && metadata.call_range.intersect(range).is_some_and(|range| !range.is_empty())
            }) {
                return None;
            }
            let expansion = db.macro_expansion(macro_file);
            Some(ExpandedMacro { metadata, text: expansion.text.clone() })
        })
        .collect::<Vec<_>>();
    if expansions.is_empty() {
        return None;
    }

    let ranges =
        expansions.iter().map(|expansion| expansion.metadata.call_range).collect::<Vec<_>>();
    let range = covering_range(&ranges).unwrap_or_else(|| TextRange::empty(offset));
    let markup = expanded_macro_markup(db, &expansions);
    Some(RangeInfo::new(range, markup))
}

struct ExpandedMacro {
    metadata: MacroFileExpansion,
    text: String,
}

fn expanded_macro_markup(db: &RootDb, expansions: &[ExpandedMacro]) -> Markup {
    let mut markup = Markup::new();

    for expansion in expansions {
        render_expanded_macro(db, &mut markup, expansion);
    }

    markup
}

fn render_expanded_macro(db: &RootDb, markup: &mut Markup, expansion: &ExpandedMacro) {
    if !markup.is_empty() {
        markup.newline();
    }
    render_macro_expansion_header(markup, &expansion.metadata.definition);
    render_macro_expansion_separator(markup);
    markup.print("Expands to");
    markup.newline();
    markup.push_with_code_fence(&macro_expansion_hover_text(expansion.text.as_str()));
    render_macro_expansion_separator(markup);
    render_macro_expansion_source_link(
        db,
        markup,
        &expansion.metadata.definition,
        expansion.metadata.call_file_id,
    );
}

pub(in crate::hover) fn macro_expansion_hover_text(text: &str) -> String {
    let lines = text.lines().collect::<Vec<_>>();
    let start = lines.iter().position(|line| !line.trim().is_empty()).unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map(|index| index + 1)
        .unwrap_or(start);
    let lines = &lines[start..end];

    let common_indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| leading_indent(line))
        .reduce(common_whitespace_prefix)
        .unwrap_or_default();

    lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                ""
            } else {
                line.strip_prefix(common_indent).unwrap_or(line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn leading_indent(line: &str) -> &str {
    let end = line
        .char_indices()
        .find_map(|(index, ch)| (!matches!(ch, ' ' | '\t')).then_some(index))
        .unwrap_or(line.len());
    &line[..end]
}

fn common_whitespace_prefix<'a>(left: &'a str, right: &'a str) -> &'a str {
    let end = left.bytes().zip(right.bytes()).take_while(|(left, right)| left == right).count();
    &left[..end]
}

fn covering_range(ranges: &[TextRange]) -> Option<TextRange> {
    let start = ranges.iter().map(|range| range.start()).min()?;
    let end = ranges.iter().map(|range| range.end()).max()?;
    Some(TextRange::new(start, end))
}
