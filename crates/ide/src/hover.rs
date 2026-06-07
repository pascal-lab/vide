use hir::{
    base_db::source_db::{SourceDb, SourceRootDb},
    container::InContainer,
    file::HirFileId,
    hir_def::expr::Expr,
    preproc::{
        EmittedTokenProvenance, IncludeTarget, MacroArgument, MacroDefinition,
        MacroExpansionProvenance, MacroExpansionUnavailable, MacroParamDefinition,
        RecursiveMacroExpansionProvenance, include_directives_at, macro_definition_at,
        macro_param_definition_at, macro_param_reference_definitions_at,
        macro_reference_definitions_at, recursive_macro_expansion_provenances_at,
    },
    semantics::Semantics,
};
use syntax::{
    SyntaxTokenWithParent, TokenKind,
    ast::{self, AstNode},
    token::TokenKindExt,
};
use utils::{
    get::GetRef,
    line_index::{TextRange, TextSize},
};
use vfs::FileId;

use crate::{
    FilePosition, RangeInfo, db::root_db::RootDb, definitions::DefinitionClass, markup::Markup,
    render,
};

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
    if let Some(macro_hover) = handle_preproc_macro(db, file_id, offset) {
        return Some(with_expanded_macro_hover(db, file_id, offset, macro_hover));
    }

    if let Some(include) = handle_preproc_include(db, file_id, offset) {
        return Some(include);
    }

    let sema = Semantics::new(db);
    let hir_file_id = file_id.into();
    let parsed_file = sema.parse_file(file_id);
    let root = parsed_file.root()?;
    let selection = crate::source_tokens::token_candidates_at_offset(
        db,
        file_id,
        root,
        offset,
        token_precedence,
    )?;
    let markups = selection
        .tokens
        .into_iter()
        .filter_map(|token| hover_for_token(&sema, hir_file_id, token))
        .collect::<Vec<_>>();
    let res = merge_hover_results(markups)?;
    Some(with_expanded_macro_hover(db, file_id, offset, RangeInfo::new(selection.range, res)))
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
    let Some(expanded) = expanded_macro_hover(db, file_id, offset) else {
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
) -> Option<RangeInfo<Markup>> {
    let expansions =
        recursive_macro_expansion_provenances_at(db, file_id, offset).ok().unwrap_or_default();
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
    markup.print("Macro expansion");

    for (index, expansion) in expansions.iter().enumerate() {
        if expansions.len() > 1 {
            markup.newline();
            markup.print("Context ");
            markup.print(&(index + 1).to_string());
        }
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
        render_unavailable_expansion(db, markup, &expansion.unavailable);
        return;
    };

    markup.newline();
    markup.print("Signature");
    markup.newline();
    render_signature_line(db, markup, &root.expansion.definition);

    if !root.expansion.call.arguments.is_empty() {
        markup.newline();
        markup.print("Arguments");
        render_arguments(db, markup, &root.expansion.definition, &root.expansion.call.arguments);
    }

    markup.newline();
    markup.print("Expanded result");
    markup.newline();
    markup.push_with_code_fence(&expanded_text_from_tokens(&root.tokens));

    markup.print("Expansion steps");
    for (index, step) in expansion.expansions.iter().enumerate() {
        render_expansion_step(db, markup, index + 1, step);
    }
    render_unavailable_expansion(db, markup, &expansion.unavailable);
}

fn render_signature_line(db: &RootDb, markup: &mut Markup, definition: &MacroDefinition) {
    markup.push_with_backticks(&macro_signature(definition));
    if let Some(source) = macro_definition_source_label(db, definition) {
        markup.print(" from ");
        markup.push_with_backticks(&source);
    }
}

fn render_arguments(
    db: &RootDb,
    markup: &mut Markup,
    definition: &MacroDefinition,
    arguments: &[MacroArgument],
) {
    for argument in arguments {
        markup.print("\n- ");
        let name = definition
            .params
            .as_ref()
            .and_then(|params| params.get(argument.argument_index))
            .and_then(|param| param.name.as_ref())
            .map_or_else(|| format!("${}", argument.argument_index), ToString::to_string);
        markup.push_with_backticks(&name);
        markup.print(" = ");
        markup.push_with_backticks(&argument_text(db, argument));
    }
}

fn render_expansion_step(
    db: &RootDb,
    markup: &mut Markup,
    index: usize,
    provenance: &MacroExpansionProvenance,
) {
    markup.newline();
    if let Some(call_text) =
        text_at_file_range(db, provenance.expansion.call.file_id, provenance.expansion.call.range)
    {
        markup.print(&index.to_string());
        markup.print(". ");
        markup.push_with_backticks(call_text.trim());
        markup.print(" from ");
        markup.push_with_backticks(&macro_signature(&provenance.expansion.definition));
        if let Some(source) = macro_definition_source_label(db, &provenance.expansion.definition) {
            markup.print(" in ");
            markup.push_with_backticks(&source);
        }
    } else {
        markup.print(&index.to_string());
        markup.print(". Expansion from ");
        markup.push_with_backticks(&macro_signature(&provenance.expansion.definition));
    }
    markup.newline();
    markup.push_with_code_fence(&expanded_text_from_tokens(&provenance.tokens));
}

fn render_unavailable_expansion(
    db: &RootDb,
    markup: &mut Markup,
    unavailable: &[MacroExpansionUnavailable],
) {
    for unavailable in unavailable {
        markup.newline();
        if let Some(call_text) =
            text_at_file_range(db, unavailable.call.file_id, unavailable.call.range)
        {
            markup.push_with_backticks(call_text.trim());
            markup.print(" expansion unavailable.");
        } else {
            markup.print("Expansion unavailable.");
        }
    }
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

fn macro_definition_source_label(db: &RootDb, definition: &MacroDefinition) -> Option<String> {
    match &definition.source {
        hir::preproc::MappedPreprocSource::RealFile { file_id } => {
            db.file_path(*file_id).map(|path| path.to_string()).or_else(|| {
                db.source_root(db.source_root_id(*file_id))
                    .path_for_file(file_id)
                    .map(|path| path.to_string())
            })
        }
        hir::preproc::MappedPreprocSource::VirtualFile { .. }
        | hir::preproc::MappedPreprocSource::VirtualDisplay { .. } => None,
    }
}

fn argument_text(db: &RootDb, argument: &MacroArgument) -> String {
    if let (Some(source), Some(range)) = (&argument.source, argument.range)
        && let Some(file_id) = source.file_id()
        && let Some(text) = text_at_file_range(db, file_id, range)
    {
        return text.trim().to_owned();
    }
    argument.tokens.iter().map(|token| token.as_str()).collect::<Vec<_>>().join(" ")
}

fn expanded_text_from_tokens(tokens: &[EmittedTokenProvenance]) -> String {
    let mut text = String::new();
    for (index, token) in tokens.iter().enumerate() {
        if index > 0 {
            text.push(' ');
        }
        text.push_str(token.text.as_str());
    }
    text
}

fn text_at_file_range(db: &RootDb, file_id: FileId, range: TextRange) -> Option<String> {
    let text = db.file_text(file_id);
    let start = usize::from(range.start());
    let end = usize::from(range.end());
    text.get(start..end).map(ToOwned::to_owned)
}

fn covering_range(ranges: &[TextRange]) -> Option<TextRange> {
    let start = ranges.iter().map(|range| range.start()).min()?;
    let end = ranges.iter().map(|range| range.end()).max()?;
    Some(TextRange::new(start, end))
}

fn handle_preproc_macro(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<RangeInfo<Markup>> {
    if let Ok(Some(definition)) = macro_param_definition_at(db, file_id, offset) {
        return Some(RangeInfo::new(definition.range, macro_param_definition_markup(&definition)));
    }

    if let Ok(Some(param_resolution)) = macro_param_reference_definitions_at(db, file_id, offset) {
        if param_resolution.definitions.is_empty() {
            return None;
        }
        return Some(RangeInfo::new(
            param_resolution.range,
            macro_param_definitions_markup(&param_resolution.definitions),
        ));
    }

    if let Ok(Some(definition)) = macro_definition_at(db, file_id, offset) {
        return Some(RangeInfo::new(
            definition.name_range,
            macro_definition_markup(db, &definition),
        ));
    }

    if let Ok(Some(resolution)) = macro_reference_definitions_at(db, file_id, offset) {
        if resolution.definitions.is_empty() {
            return None;
        }
        return Some(RangeInfo::new(
            resolution.range,
            macro_definitions_markup(db, &resolution.definitions),
        ));
    }

    None
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

fn macro_definition_markup(db: &RootDb, definition: &MacroDefinition) -> Markup {
    macro_definitions_markup(db, std::slice::from_ref(definition))
}

fn macro_definitions_markup(db: &RootDb, definitions: &[MacroDefinition]) -> Markup {
    let mut markup = Markup::new();
    if definitions.len() == 1 {
        markup.print("Macro");
        markup.newline();
        markup.push_with_backticks(definitions[0].name.as_str());
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

fn handle_preproc_include(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<RangeInfo<Markup>> {
    let includes = include_directives_at(db, file_id, offset).ok()?;
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
