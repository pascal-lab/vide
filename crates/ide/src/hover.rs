use hir::{
    base_db::source_db::SourceDb,
    container::InContainer,
    file::HirFileId,
    hir_def::expr::Expr,
    preproc::{
        IncludeTarget, MacroDefinition, include_directive_at, macro_definition_at,
        macro_reference_definitions_at,
    },
    semantics::Semantics,
};
use syntax::{
    SyntaxNodeExt, SyntaxTokenWithParent, TokenKind,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
    token::TokenKindExt,
};
use utils::{get::GetRef, line_index::TextSize};
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
        return Some(macro_hover);
    }

    if let Some(include) = handle_preproc_include(db, file_id, offset) {
        return Some(include);
    }

    let sema = Semantics::new(db);
    let hir_file_id = file_id.into();
    let parsed_file = sema.parse_file(file_id);
    let root = parsed_file.root()?;
    let token = root.token_at_offset(offset).pick_bext_token(token_precedence)?;

    let res = handle_literal(&sema, hir_file_id, token)
        .or_else(|| handle_definition(&sema, hir_file_id, token))?;
    Some(RangeInfo::new(token.text_range()?, res))
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

fn handle_preproc_macro(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<RangeInfo<Markup>> {
    if let Some(definition) = macro_definition_at(db, file_id, offset).ok()? {
        return Some(RangeInfo::new(definition.name_range, macro_definition_markup(&definition)));
    }

    let resolution = macro_reference_definitions_at(db, file_id, offset).ok()??;
    let definition = resolution.definitions.into_iter().next()?;
    Some(RangeInfo::new(resolution.reference.range, macro_definition_markup(&definition)))
}

fn macro_definition_markup(definition: &MacroDefinition) -> Markup {
    let mut markup = Markup::new();
    markup.print("Macro");
    markup.newline();
    markup.push_with_backticks(definition.name.as_str());
    markup
}

fn handle_preproc_include(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<RangeInfo<Markup>> {
    let include = include_directive_at(db, file_id, offset).ok()??;
    let mut markup = Markup::new();
    match include.target {
        IncludeTarget::Literal { path, resolved_file } => {
            markup.print("Include");
            markup.newline();
            markup.push_with_backticks(path.as_str());
            if let Some(target_file_id) = resolved_file
                && let Some(path) = db.file_path(target_file_id)
            {
                markup.newline();
                markup.print(&path.to_string());
            }
        }
        IncludeTarget::Token { raw } => {
            markup.print("Include");
            markup.newline();
            markup.push_with_backticks(raw.as_str());
        }
    }
    Some(RangeInfo::new(include.range, markup))
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
