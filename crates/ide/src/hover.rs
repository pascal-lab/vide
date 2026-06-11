use hir::{
    base_db::{
        source_db::{SourceDb, SourceRootDb},
        source_root::SourceRootRole,
    },
    container::InContainer,
    file::HirFileId,
    hir_def::expr::Expr,
    preproc::{
        IncludeDirective, IncludeTarget, MacroDefinition, MacroExpansionDefinition,
        MacroParamDefinition, MacroParamReferenceDefinitions, MacroReferenceDefinitions,
        RecursiveMacroExpansionProvenance, include_directives_at, macro_definition_at,
        macro_param_definition_at, macro_param_reference_definitions_at,
        macro_reference_definitions_at, recursive_macro_expansion_provenances_at,
    },
    semantics::Semantics,
};
use syntax::{
    SyntaxNode, SyntaxTokenWithParent, TokenKind,
    ast::{self, AstNode},
    token::TokenKindExt,
};
use utils::{
    get::GetRef,
    line_index::{TextRange, TextSize},
};
use vfs::FileId;

use crate::{
    FilePosition, RangeInfo,
    db::root_db::RootDb,
    definitions::DefinitionClass,
    markup::Markup,
    render,
    source_targets::{SourceTarget, source_target_at_offset},
};

const MACRO_EXPANSION_SEPARATOR: &str = "--------------------";

struct MacroSourceLink {
    label: String,
    target: String,
}

enum HoverTarget<'tree> {
    Macro(Box<MacroHoverTarget>),
    Include(Vec<IncludeDirective>),
    Source(SourceTarget<'tree>),
}

enum MacroHoverTarget {
    ParamDefinition(MacroParamDefinition),
    ParamReference(MacroParamReferenceDefinitions),
    Definition(MacroDefinition),
    Reference(MacroReferenceDefinitions),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverFormat {
    Markdown,
    PlainText,
}

#[derive(Debug, Clone)]
pub struct HoverConfig {
    pub format: HoverFormat,
}

pub(crate) fn hover(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
    _config: HoverConfig,
) -> Option<RangeInfo<Markup>> {
    let sema = Semantics::new(db);
    let parsed_file = sema.parse_file(file_id);
    let target = dispatch_hover_target(db, file_id, offset, parsed_file.root())?;
    render_hover_target(db, file_id, offset, &sema, target)
}

fn dispatch_hover_target<'tree>(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
    root: Option<SyntaxNode<'tree>>,
) -> Option<HoverTarget<'tree>> {
    if let Some(macro_target) = dispatch_macro_hover_target(db, file_id, offset) {
        return Some(HoverTarget::Macro(Box::new(macro_target)));
    }
    if let Some(includes) = dispatch_include_hover_target(db, file_id, offset) {
        return Some(HoverTarget::Include(includes));
    }
    let root = root?;
    let target =
        source_target_at_offset(db, file_id, root, offset, token_precedence)?.resolved()?;
    Some(HoverTarget::Source(target))
}

fn render_hover_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
    sema: &Semantics<RootDb>,
    target: HoverTarget<'_>,
) -> Option<RangeInfo<Markup>> {
    match target {
        HoverTarget::Macro(target) => render_macro_hover_target(db, file_id, offset, *target),
        HoverTarget::Include(includes) => render_include_hover(db, includes),
        HoverTarget::Source(target) => {
            let hover = hover_for_source_target(sema, file_id.into(), target)?;
            Some(with_expanded_macro_hover(db, file_id, offset, hover))
        }
    }
}

fn hover_for_source_target(
    sema: &Semantics<RootDb>,
    hir_file_id: HirFileId,
    target: SourceTarget<'_>,
) -> Option<RangeInfo<Markup>> {
    let (range, tokens) = target.into_parts();
    hover_for_token_selection(sema, hir_file_id, range, tokens)
}

fn hover_for_token_selection(
    sema: &Semantics<RootDb>,
    hir_file_id: HirFileId,
    range: TextRange,
    tokens: Vec<SyntaxTokenWithParent<'_>>,
) -> Option<RangeInfo<Markup>> {
    let markups = tokens
        .into_iter()
        .filter_map(|token| hover_for_token(sema, hir_file_id, token))
        .collect::<Vec<_>>();
    let res = merge_hover_results(markups)?;
    Some(RangeInfo::new(range, res))
}

pub(crate) fn token_precedence(kind: TokenKind) -> usize {
    match kind {
        _ if kind.name_like() => 4,
        _ if kind.is_literal() => 3,
        _ => 1,
    }
}

fn handle_literal(
    sema: &Semantics<RootDb>,
    file_id: HirFileId,
    SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<Markup> {
    if !tok.kind().is_literal() {
        return None;
    }

    let expr = ast::Expression::cast(parent)?;
    let InContainer { value: expr_id, cont_id } = sema.resolve_expr(file_id, expr)?;
    let container = cont_id.to_container(sema.db);
    let Expr::Literal(literal) = container.get(expr_id) else {
        return None;
    };

    render::render_literal(literal)
}

fn hover_for_token(
    sema: &Semantics<RootDb>,
    file_id: HirFileId,
    token: SyntaxTokenWithParent,
) -> Option<Markup> {
    handle_literal(sema, file_id, token).or_else(|| handle_definition(sema, file_id, token))
}

fn merge_hover_results(markups: Vec<Markup>) -> Option<Markup> {
    let mut iter = markups.into_iter();
    let mut res = iter.next()?;
    for markup in iter {
        res.horizontal_line();
        res.merge(markup);
    }
    Some(res)
}

fn with_expanded_macro_hover(
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

fn expanded_macro_hover(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
    reference_definitions: Option<&MacroReferenceDefinitions>,
) -> Option<RangeInfo<Markup>> {
    let reference_ids = if let Some(reference_definitions) = reference_definitions {
        reference_definitions.references.iter().map(|reference| reference.id).collect::<Vec<_>>()
    } else {
        macro_reference_definitions_at(db, file_id, offset)
            .ok()
            .flatten()?
            .references
            .into_iter()
            .map(|reference| reference.id)
            .collect::<Vec<_>>()
    };
    if reference_ids.is_empty() {
        return None;
    }

    let expansions =
        recursive_macro_expansion_provenances_at(db, file_id, offset).ok().unwrap_or_default();
    let expansions = expansions
        .into_iter()
        .filter(|expansion| {
            reference_ids.contains(&expansion.root_call.reference_id)
                && !expansion.expansions.is_empty()
        })
        .collect::<Vec<_>>();
    if expansions.is_empty() {
        return None;
    }

    let ranges = expansions.iter().map(|expansion| expansion.root_call.range).collect::<Vec<_>>();
    let range = covering_range(&ranges).unwrap_or_else(|| TextRange::empty(offset));
    let markup = expanded_macro_markup(db, &expansions);
    Some(RangeInfo::new(range, markup))
}

fn expanded_macro_markup(db: &RootDb, expansions: &[RecursiveMacroExpansionProvenance]) -> Markup {
    let mut markup = Markup::new();

    for expansion in expansions {
        render_recursive_expansion(db, &mut markup, expansion);
    }

    markup
}

fn render_recursive_expansion(
    db: &RootDb,
    markup: &mut Markup,
    expansion: &RecursiveMacroExpansionProvenance,
) {
    let Some(root) = expansion.expansions.first() else {
        return;
    };

    if !markup.is_empty() {
        markup.newline();
    }
    render_macro_expansion_header(markup, &root.expansion.definition);
    render_macro_expansion_separator(markup);
    markup.print("Expands to");
    markup.newline();
    markup.push_with_code_fence(&macro_expansion_hover_text(root.expansion.display_text.as_str()));
    render_macro_expansion_separator(markup);
    if let MacroExpansionDefinition::Source(definition) = &root.expansion.definition {
        render_macro_source_link(db, markup, definition, root.expansion.call.file_id);
    }
}

fn render_macro_expansion_header(markup: &mut Markup, definition: &MacroExpansionDefinition) {
    match definition {
        MacroExpansionDefinition::Source(definition) => {
            markup.push_with_code_fence(&macro_signature(definition));
        }
        MacroExpansionDefinition::Builtin { name, .. } => {
            markup.push_with_code_fence(&format!("`{name}"));
        }
    }
}

fn render_macro_expansion_separator(markup: &mut Markup) {
    markup.newline();
    markup.print(MACRO_EXPANSION_SEPARATOR);
    markup.newline();
}

fn macro_signature(definition: &MacroDefinition) -> String {
    let mut signature = format!("`{}", definition.name);
    if let Some(params) = &definition.params {
        signature.push('(');
        for (index, param) in params.iter().enumerate() {
            if index > 0 {
                signature.push_str(", ");
            }
            signature.push_str(param.name.as_deref().unwrap_or("<unnamed>"));
        }
        signature.push(')');
    }
    signature
}

fn macro_definition_line(definition: &MacroDefinition) -> String {
    let mut line = String::from("`define ");
    line.push_str(&macro_signature(definition));
    let body = macro_definition_body_text(definition);
    if !body.is_empty() {
        line.push(' ');
        line.push_str(&body);
    }
    line
}

fn macro_definition_source_link(
    db: &RootDb,
    definition: &MacroDefinition,
    anchor_file_id: FileId,
) -> Option<MacroSourceLink> {
    match &definition.source {
        hir::preproc::MappedPreprocSource::RealFile { file_id } => {
            macro_file_source_link(db, *file_id, anchor_file_id)
        }
        hir::preproc::MappedPreprocSource::VirtualFile { .. }
        | hir::preproc::MappedPreprocSource::VirtualDisplay { .. } => None,
    }
}

fn macro_file_source_link(
    db: &RootDb,
    file_id: FileId,
    anchor_file_id: FileId,
) -> Option<MacroSourceLink> {
    let source_root = db.source_root(db.source_root_id(file_id));
    let label = if matches!(source_root.role(), SourceRootRole::Local)
        && let Some(label) = local_source_root_path_label(db, file_id, anchor_file_id)
    {
        label
    } else {
        source_root
            .path_for_file(&file_id)
            .map(|path| display_hover_path(path.to_string()))
            .or_else(|| db.file_path(file_id).map(|path| display_hover_path(path.to_string())))?
    };
    let target = db
        .file_path(file_id)
        .map(|path| file_link_target(&path.to_string()))
        .unwrap_or_else(|| label.clone());
    Some(MacroSourceLink { label, target })
}

fn local_source_root_path_label(
    db: &RootDb,
    file_id: FileId,
    anchor_file_id: FileId,
) -> Option<String> {
    let source_root = db.source_root(db.source_root_id(file_id));
    let source_path = source_root.path_for_file(&file_id)?;
    let Some(target_path) = source_path.as_abs_path() else {
        return Some(display_project_path(source_path.to_string()));
    };

    let anchor_source_root = db.source_root(db.source_root_id(anchor_file_id));
    let anchor_path = anchor_source_root.path_for_file(&anchor_file_id)?.as_abs_path()?;
    let mut common_dir = anchor_path.parent()?.to_path_buf();
    while !target_path.starts_with(common_dir.as_path()) {
        if !common_dir.pop() {
            return None;
        }
    }
    if !has_normal_path_component(common_dir.as_path()) {
        return None;
    }

    target_path
        .strip_prefix(common_dir.as_path())
        .map(|path| display_project_path(path.as_ref().display().to_string()))
}

fn has_normal_path_component(path: &utils::paths::AbsPath) -> bool {
    path.components().any(|component| matches!(component, utils::paths::Utf8Component::Normal(_)))
}

fn display_project_path(mut path: String) -> String {
    while path.starts_with('/') {
        path.remove(0);
    }
    display_hover_path(path)
}

fn display_hover_path(path: String) -> String {
    path.replace('\\', "/")
}

fn file_link_target(path: &str) -> String {
    let path = display_hover_path(path.to_owned());
    if path.starts_with('/') { format!("file://{path}") } else { format!("file:///{path}") }
}

fn macro_expansion_hover_text(text: &str) -> String {
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

fn dispatch_macro_hover_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<MacroHoverTarget> {
    if let Ok(Some(definition)) = macro_param_definition_at(db, file_id, offset) {
        return Some(MacroHoverTarget::ParamDefinition(definition));
    }

    if let Ok(Some(param_resolution)) = macro_param_reference_definitions_at(db, file_id, offset) {
        if param_resolution.definitions.is_empty() {
            return None;
        }
        return Some(MacroHoverTarget::ParamReference(param_resolution));
    }

    if let Ok(Some(definition)) = macro_definition_at(db, file_id, offset) {
        return Some(MacroHoverTarget::Definition(definition));
    }

    if let Ok(Some(resolution)) = macro_reference_definitions_at(db, file_id, offset) {
        return Some(MacroHoverTarget::Reference(resolution));
    }

    None
}

fn render_macro_hover_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
    target: MacroHoverTarget,
) -> Option<RangeInfo<Markup>> {
    match target {
        MacroHoverTarget::ParamDefinition(definition) => {
            Some(RangeInfo::new(definition.range, macro_param_definition_markup(&definition)))
        }
        MacroHoverTarget::ParamReference(param_resolution) => Some(RangeInfo::new(
            param_resolution.range,
            macro_param_definitions_markup(&param_resolution.definitions),
        )),
        MacroHoverTarget::Definition(definition) => Some(RangeInfo::new(
            definition.name_range,
            macro_definition_markup(db, file_id, &definition),
        )),
        MacroHoverTarget::Reference(resolution) => {
            if resolution.definitions.is_empty() {
                return expanded_macro_hover(db, file_id, offset, Some(&resolution));
            }
            expanded_macro_hover(db, file_id, offset, Some(&resolution)).or_else(|| {
                Some(RangeInfo::new(
                    resolution.range,
                    macro_definitions_markup(db, file_id, &resolution.definitions),
                ))
            })
        }
    }
}

fn macro_param_definition_markup(definition: &MacroParamDefinition) -> Markup {
    macro_param_definitions_markup(std::slice::from_ref(definition))
}

fn macro_param_definitions_markup(definitions: &[MacroParamDefinition]) -> Markup {
    let mut markup = Markup::new();
    if definitions.len() == 1 {
        markup.print("Macro parameter");
        markup.newline();
        markup.push_with_backticks(definitions[0].name.as_str());
        markup.print(" of ");
        markup.push_with_backticks(definitions[0].macro_definition.name.as_str());
        return markup;
    }

    markup.print("Macro parameters");
    for definition in definitions {
        markup.newline();
        markup.push_with_backticks(definition.name.as_str());
        markup.print(" of ");
        markup.push_with_backticks(definition.macro_definition.name.as_str());
    }
    markup
}

fn macro_definition_markup(
    db: &RootDb,
    anchor_file_id: FileId,
    definition: &MacroDefinition,
) -> Markup {
    macro_definitions_markup(db, anchor_file_id, std::slice::from_ref(definition))
}

fn macro_definitions_markup(
    db: &RootDb,
    anchor_file_id: FileId,
    definitions: &[MacroDefinition],
) -> Markup {
    let mut markup = Markup::new();
    if definitions.len() == 1 {
        render_macro_definition_display(db, &mut markup, anchor_file_id, &definitions[0]);
        return markup;
    }

    markup.print("Macro definitions");
    for definition in definitions {
        markup.newline();
        markup.push_with_backticks(definition.name.as_str());
        if let Some(path) = db.file_path(definition.file_id) {
            markup.print(" ");
            markup.print(&path.to_string());
        }
    }
    markup
}

fn render_macro_definition_display(
    db: &RootDb,
    markup: &mut Markup,
    anchor_file_id: FileId,
    definition: &MacroDefinition,
) {
    markup.push_with_code_fence(&macro_definition_line(definition));
    render_macro_expansion_separator(markup);
    render_macro_source_link(db, markup, definition, anchor_file_id);
}

fn render_macro_source_link(
    db: &RootDb,
    markup: &mut Markup,
    definition: &MacroDefinition,
    anchor_file_id: FileId,
) {
    let Some(source) = macro_definition_source_link(db, definition, anchor_file_id) else {
        return;
    };
    markup.print_with_strong("Macro");
    markup.print(" from [");
    markup.print(&markdown_link_label(&source.label));
    markup.print("](<");
    markup.print(&markdown_link_destination(&source.target));
    markup.print(">)");
}

fn markdown_link_label(label: &str) -> String {
    label.replace('\\', "\\\\").replace('[', "\\[").replace(']', "\\]")
}

fn markdown_link_destination(destination: &str) -> String {
    destination.replace('>', "%3E")
}

fn macro_definition_body_text(definition: &MacroDefinition) -> String {
    definition.body_tokens.iter().map(|token| token.as_str()).collect::<Vec<_>>().join(" ")
}

fn dispatch_include_hover_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<Vec<IncludeDirective>> {
    let includes = include_directives_at(db, file_id, offset).ok()?;
    (!includes.is_empty()).then_some(includes)
}

fn render_include_hover(db: &RootDb, includes: Vec<IncludeDirective>) -> Option<RangeInfo<Markup>> {
    let range = includes.first()?.range;
    let mut markup = Markup::new();
    markup.print("Include");
    for include in includes {
        markup.newline();
        match include.target {
            IncludeTarget::Literal { path, resolved_file } => {
                markup.push_with_backticks(path.as_str());
                if let Some(target_file_id) = resolved_file
                    && let Some(path) = db.file_path(target_file_id)
                {
                    markup.newline();
                    markup.print(&path.to_string());
                }
            }
            IncludeTarget::Token { raw } => {
                markup.push_with_backticks(raw.as_str());
            }
        }
    }
    Some(RangeInfo::new(range, markup))
}

fn handle_definition(
    sema: &Semantics<RootDb>,
    file_id: HirFileId,
    tp: SyntaxTokenWithParent,
) -> Option<Markup> {
    let def = DefinitionClass::resolve(sema, file_id, tp)?;
    let mut res = Markup::new();

    match def {
        DefinitionClass::Definition(def) => {
            res.merge(render::render_definition(sema, def));
        }
        DefinitionClass::PortConnShorthand { port, local } => {
            res.new_subsection("Port");
            res.merge(render::render_definition(sema, port));
            res.horizontal_line();
            res.new_subsection("Local");
            res.merge(render::render_definition(sema, local));
        }
        DefinitionClass::Ambiguous(definitions) => {
            res.print("Ambiguous reference");
            for definition in definitions {
                res.horizontal_line();
                res.merge(render::render_definition_location(sema, definition));
            }
        }
    }

    Some(res)
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;

    #[test]
    fn macro_expansion_hover_text_matrix() {
        let mut report = String::new();

        for (name, text) in [
            (
                "dedents common indentation",
                "\n    always_ff @(posedge clk) begin\n      q <= 1;\n    end\n",
            ),
            ("removes single-line callsite indent", "  logic generated;"),
        ] {
            writeln!(&mut report, "{name}:").unwrap();
            writeln!(&mut report, "{}", macro_expansion_hover_text(text)).unwrap();
        }

        insta::assert_snapshot!(report);
    }
}
