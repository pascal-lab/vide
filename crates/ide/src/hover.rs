use hir::{
    base_db::{
        source_db::{SourceDb, SourceRootDb},
        source_root::SourceRootRole,
    },
    container::InContainer,
    file::HirFileId,
    hir_def::expr::Expr,
    preproc::{
        MacroDefinition, MacroExpansionDefinition, RecursiveMacroExpansionProvenance,
        macro_reference_definitions_at, recursive_macro_expansion_provenances_at,
    },
    semantics::Semantics,
    source_resolver::PositionResolver,
};
use source_model::{
    FilePosition as SourceFilePosition, ResolvedSourceTarget, SourceContext, SourceEntity,
    SourcePurpose, SourceRangeResult, SourceTarget as GraphSourceTarget,
    SourceTargetResolution as GraphSourceTargetResolution,
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
    Graph(RangeInfo<Markup>),
    Source(SourceTarget<'tree>),
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
    if let Some(target) = dispatch_source_graph_hover_target(db, file_id, offset) {
        return Some(target);
    }
    let root = root?;
    let target =
        source_target_at_offset(db, file_id, root, offset, token_precedence)?.resolved()?;
    Some(HoverTarget::Source(target))
}

fn dispatch_source_graph_hover_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<HoverTarget<'static>> {
    let resolution = PositionResolver::new(db).resolve_position(
        SourceFilePosition { file_id, offset },
        SourcePurpose::Hover,
        None,
    );

    match resolution {
        GraphSourceTargetResolution::Resolved(target) => {
            dispatch_graph_hover_target(db, file_id, offset, target)
        }
        GraphSourceTargetResolution::Ambiguous(targets) => targets
            .into_iter()
            .find_map(|target| dispatch_graph_hover_target(db, file_id, offset, target)),
        GraphSourceTargetResolution::Blocked(_) | GraphSourceTargetResolution::None => None,
    }
}

fn dispatch_graph_hover_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
    target: ResolvedSourceTarget,
) -> Option<HoverTarget<'static>> {
    match target.target {
        GraphSourceTarget::MacroParamDefinition(_) => {
            dispatch_graph_macro_param_definition_hover_target(db, file_id, target)
                .map(HoverTarget::Graph)
        }
        GraphSourceTarget::MacroParamReference(_) => {
            dispatch_graph_macro_param_reference_hover_target(db, file_id, target)
                .map(HoverTarget::Graph)
        }
        GraphSourceTarget::MacroDefinition(_) => {
            dispatch_graph_macro_definition_hover_target(db, file_id, target)
                .map(HoverTarget::Graph)
        }
        GraphSourceTarget::MacroReference(_) => {
            dispatch_graph_macro_reference_hover_target(db, file_id, offset, target)
                .map(HoverTarget::Graph)
        }
        GraphSourceTarget::Include(id) => {
            dispatch_graph_include_hover_target(db, file_id, target, id).map(HoverTarget::Graph)
        }
        GraphSourceTarget::MacroCall(_)
        | GraphSourceTarget::ExpansionToken(_)
        | GraphSourceTarget::HirSymbol(_)
        | GraphSourceTarget::HirReference(_)
        | GraphSourceTarget::SyntaxToken(_) => None,
    }
}

fn render_hover_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
    sema: &Semantics<RootDb>,
    target: HoverTarget<'_>,
) -> Option<RangeInfo<Markup>> {
    match target {
        HoverTarget::Graph(hover) => Some(hover),
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
    reference_ids: Option<&[usize]>,
) -> Option<RangeInfo<Markup>> {
    let reference_ids = if let Some(reference_ids) = reference_ids {
        reference_ids.to_vec()
    } else {
        macro_reference_definitions_at(db, file_id, offset)
            .ok()
            .flatten()?
            .references
            .into_iter()
            .map(|reference| reference.id.raw())
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
            reference_ids.contains(&expansion.root_call.reference_id.raw())
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

fn dispatch_graph_macro_param_definition_hover_target(
    db: &RootDb,
    file_id: FileId,
    definition: ResolvedSourceTarget,
) -> Option<RangeInfo<Markup>> {
    let source_graph = db.source_graph_preproc_model(file_id);
    let source_graph = source_graph.as_ref().as_ref().ok()?;
    let graph = &source_graph.graph;
    let SourceRangeResult::Mapped(definition_range) =
        graph.entity_focus_file_range(definition.entity, SourcePurpose::Hover)
    else {
        return None;
    };
    Some(RangeInfo::new(
        definition_range.range,
        graph_macro_param_definitions_markup(db, graph, &[definition.entity]),
    ))
}

fn dispatch_graph_macro_param_reference_hover_target(
    db: &RootDb,
    file_id: FileId,
    reference: ResolvedSourceTarget,
) -> Option<RangeInfo<Markup>> {
    let source_graph = db.source_graph_preproc_model(file_id);
    let source_graph = source_graph.as_ref().as_ref().ok()?;
    let graph = &source_graph.graph;
    let SourceRangeResult::Mapped(reference_range) =
        graph.entity_focus_file_range(reference.entity, SourcePurpose::Hover)
    else {
        return None;
    };
    let definitions = graph
        .resolved_definitions(source_graph.root_context, reference.entity)
        .iter()
        .map(|(definition, _)| *definition)
        .collect::<Vec<_>>();
    (!definitions.is_empty()).then_some(RangeInfo::new(
        reference_range.range,
        graph_macro_param_definitions_markup(db, graph, &definitions),
    ))
}

fn graph_macro_param_definitions_markup(
    db: &RootDb,
    graph: &source_model::SourceGraph,
    definitions: &[source_model::EntityId],
) -> Markup {
    let mut markup = Markup::new();
    if definitions.len() == 1 {
        markup.print("Macro parameter");
        markup.newline();
        let (name, macro_name) = graph_macro_param_display(db, graph, definitions[0]);
        markup.push_with_backticks(name.as_str());
        markup.print(" of ");
        markup.push_with_backticks(macro_name.as_deref().unwrap_or("<unknown>"));
        return markup;
    }

    markup.print("Macro parameters");
    for definition in definitions.iter().copied() {
        let (name, macro_name) = graph_macro_param_display(db, graph, definition);
        markup.newline();
        markup.push_with_backticks(name.as_str());
        markup.print(" of ");
        markup.push_with_backticks(macro_name.as_deref().unwrap_or("<unknown>"));
    }
    markup
}

fn graph_macro_param_display(
    db: &RootDb,
    graph: &source_model::SourceGraph,
    entity: source_model::EntityId,
) -> (String, Option<String>) {
    let name = graph_entity_focus_text(db, graph, entity).unwrap_or_else(|| "<unknown>".to_owned());
    let macro_name = graph
        .entity_parents(entity)
        .iter()
        .find_map(|parent| graph_macro_definition_name(db, graph, *parent));
    (name, macro_name)
}

fn graph_macro_definition_name(
    db: &RootDb,
    graph: &source_model::SourceGraph,
    entity: source_model::EntityId,
) -> Option<String> {
    let SourceEntity::MacroDefinition(_) = graph.entity(entity) else {
        return None;
    };
    graph_entity_focus_text(db, graph, entity)
}

fn graph_entity_focus_text(
    db: &RootDb,
    graph: &source_model::SourceGraph,
    entity: source_model::EntityId,
) -> Option<String> {
    let SourceRangeResult::Mapped(file_range) =
        graph.entity_focus_file_range(entity, SourcePurpose::Hover)
    else {
        return None;
    };
    let text = db.file_text(file_range.file_id);
    Some(text[file_range.range].to_owned())
}

fn graph_entity_focus_file_id(
    graph: &source_model::SourceGraph,
    entity: source_model::EntityId,
) -> Option<FileId> {
    let SourceRangeResult::Mapped(file_range) =
        graph.entity_focus_file_range(entity, SourcePurpose::Hover)
    else {
        return None;
    };
    Some(file_range.file_id)
}

fn graph_macro_definition_line(
    db: &RootDb,
    graph: &source_model::SourceGraph,
    definition: source_model::EntityId,
) -> Option<String> {
    let SourceRangeResult::Mapped(full_range) =
        graph.entity_full_file_range(definition, SourcePurpose::Hover)
    else {
        return None;
    };
    let SourceRangeResult::Mapped(focus_range) =
        graph.entity_focus_file_range(definition, SourcePurpose::Hover)
    else {
        return None;
    };
    if full_range.file_id != focus_range.file_id {
        return None;
    }

    let text = db.file_text(full_range.file_id);
    let name = text[focus_range.range].trim();
    let suffix = text[TextRange::new(focus_range.range.end(), full_range.range.end())].trim_end();
    let mut line = format!("`define `{name}");
    if !suffix.is_empty() {
        line.push_str(suffix);
    }
    Some(line)
}

fn dispatch_graph_macro_reference_hover_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
    reference: ResolvedSourceTarget,
) -> Option<RangeInfo<Markup>> {
    let source_graph = db.source_graph_preproc_model(file_id);
    let source_graph = source_graph.as_ref().as_ref().ok()?;
    let graph = &source_graph.graph;
    let SourceRangeResult::Mapped(reference_range) =
        graph.entity_full_file_range(reference.entity, SourcePurpose::Hover)
    else {
        return None;
    };
    let GraphSourceTarget::MacroReference(reference_id) = reference.target else {
        return None;
    };
    let reference_ids = [reference_id.raw() as usize];
    if let Some(expanded) = expanded_macro_hover(db, file_id, offset, Some(&reference_ids)) {
        return Some(expanded);
    }

    let definitions = graph
        .resolved_definitions(source_graph.root_context, reference.entity)
        .iter()
        .map(|(definition, _)| *definition)
        .collect::<Vec<_>>();
    (!definitions.is_empty()).then_some(RangeInfo::new(
        reference_range.range,
        graph_macro_definitions_markup(db, graph, file_id, &definitions),
    ))
}

fn graph_macro_definitions_markup(
    db: &RootDb,
    graph: &source_model::SourceGraph,
    anchor_file_id: FileId,
    definitions: &[source_model::EntityId],
) -> Markup {
    let mut markup = Markup::new();
    if definitions.len() == 1 {
        render_graph_macro_definition_display(
            db,
            graph,
            &mut markup,
            anchor_file_id,
            definitions[0],
        );
        return markup;
    }

    markup.print("Macro definitions");
    for definition in definitions.iter().copied() {
        markup.newline();
        let name = graph_entity_focus_text(db, graph, definition)
            .unwrap_or_else(|| "<unknown>".to_owned());
        markup.push_with_backticks(name.as_str());
        if let Some(file_id) = graph_entity_focus_file_id(graph, definition)
            && let Some(path) = db.file_path(file_id)
        {
            markup.print(" ");
            markup.print(&path.to_string());
        }
    }
    markup
}

fn render_graph_macro_definition_display(
    db: &RootDb,
    graph: &source_model::SourceGraph,
    markup: &mut Markup,
    anchor_file_id: FileId,
    definition: source_model::EntityId,
) {
    let Some(line) = graph_macro_definition_line(db, graph, definition) else {
        return;
    };
    markup.push_with_code_fence(&line);
    render_macro_expansion_separator(markup);
    let Some(file_id) = graph_entity_focus_file_id(graph, definition) else {
        return;
    };
    if let Some(source) = macro_file_source_link(db, file_id, anchor_file_id) {
        render_macro_source_link_from_link(markup, source);
    }
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
    render_macro_source_link_from_link(markup, source);
}

fn render_macro_source_link_from_link(markup: &mut Markup, source: MacroSourceLink) {
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

fn dispatch_graph_macro_definition_hover_target(
    db: &RootDb,
    file_id: FileId,
    definition: ResolvedSourceTarget,
) -> Option<RangeInfo<Markup>> {
    let source_graph = db.source_graph_preproc_model(file_id);
    let source_graph = source_graph.as_ref().as_ref().ok()?;
    let graph = &source_graph.graph;
    let SourceRangeResult::Mapped(focus_range) =
        graph.entity_focus_file_range(definition.entity, SourcePurpose::Hover)
    else {
        return None;
    };
    let line = graph_macro_definition_line(db, graph, definition.entity)?;

    let mut markup = Markup::new();
    markup.push_with_code_fence(&line);
    render_macro_expansion_separator(&mut markup);
    if let Some(source) = macro_file_source_link(db, focus_range.file_id, file_id) {
        render_macro_source_link_from_link(&mut markup, source);
    }
    Some(RangeInfo::new(focus_range.range, markup))
}

fn dispatch_graph_include_hover_target(
    db: &RootDb,
    file_id: FileId,
    include: ResolvedSourceTarget,
    include_id: source_model::IncludeDirectiveId,
) -> Option<RangeInfo<Markup>> {
    let source_graph = db.source_graph_preproc_model(file_id);
    let source_graph = source_graph.as_ref().as_ref().ok()?;
    let graph = &source_graph.graph;
    let SourceRangeResult::Mapped(include_range) =
        graph.entity_focus_file_range(include.entity, SourcePurpose::Hover)
    else {
        return None;
    };
    let included_context = graph.included_context(source_graph.root_context, include_id)?;
    let SourceContext::IncludeContext { included_file, .. } = *graph.context(included_context)
    else {
        return None;
    };
    let include_text = db.file_text(include_range.file_id);
    let literal = include_text[include_range.range].to_owned();

    let mut markup = Markup::new();
    markup.print("Include");
    markup.newline();
    markup.push_with_backticks(literal.as_str());
    if let Some(path) = db.file_path(included_file) {
        markup.newline();
        markup.print(&path.to_string());
    }
    Some(RangeInfo::new(include_range.range, markup))
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
    use super::*;

    #[test]
    fn macro_expansion_hover_text_dedents_common_indentation() {
        let text = "\n    always_ff @(posedge clk) begin\n      q <= 1;\n    end\n";

        assert_eq!(
            macro_expansion_hover_text(text),
            "always_ff @(posedge clk) begin\n  q <= 1;\nend"
        );
    }

    #[test]
    fn macro_expansion_hover_text_removes_single_line_callsite_indent() {
        assert_eq!(macro_expansion_hover_text("  logic generated;"), "logic generated;");
    }
}
