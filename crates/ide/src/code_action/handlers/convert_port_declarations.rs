use std::ops::Range;

use hir::{
    base_db::source_db::SourceDb,
    container::{InContainer, InModule},
    db::HirDb,
    display::HirDisplay,
    hir_def::{
        Ident,
        declaration::DeclarationSrc,
        expr::declarator::{DeclId, DeclaratorParent},
        module::{
            Module, ModuleId, ModuleSourceMap,
            port::{PortDecl, PortDeclSrc, Ports},
        },
    },
    source_map::IsSrc,
    symbol::{NameContext, NameScope},
};
use itertools::Itertools;
use syntax::{
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
};
use utils::{
    get::{Get, GetRef},
    text_edit::TextRange,
};

use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, line_indent,
};

const ANSI_TO_NON_ANSI_ID: CodeActionId = CodeActionId {
    name: "convert_ansi_ports_to_non_ansi",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const ANSI_TO_NON_ANSI_LABEL: &str = "Convert ANSI port declarations to non-ANSI";

const NON_ANSI_TO_ANSI_ID: CodeActionId = CodeActionId {
    name: "convert_non_ansi_ports_to_ansi",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const NON_ANSI_TO_ANSI_LABEL: &str = "Convert non-ANSI port declarations to ANSI";

// Assist: convert_port_declarations
//
// This converts module ports between ANSI declarations and non-ANSI
// declarations.
//
// ```
// module top($0input a, output logic b); endmodule
// ```
// ->
// ```
// module top(a, b); input a; output logic b; endmodule
// ```
pub(super) fn convert_port_declarations(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    convert_ansi_ports_to_non_ansi(collector, ctx)
        .or(convert_non_ansi_ports_to_ansi(collector, ctx))
}

fn convert_ansi_ports_to_non_ansi(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let ast_module = ctx.find_node_at_offset::<ast::ModuleDeclaration>()?;
    let port_list = ast_module.header().ports()?.as_ansi_port_list()?;

    let module_id = ctx.sema().module_to_def(ctx.file_id().into(), ast_module)?;
    let (module, module_src_map) = ctx.sema().db.module_with_source_map(module_id);
    let Ports::Ansi(port_decls) = &module.ports else {
        return None;
    };

    let mut port_names = Vec::with_capacity(port_decls.len());
    let mut port_items = Vec::with_capacity(port_decls.len());
    for (port_id, port_decl) in port_decls.iter() {
        let src = module_src_map.port_srcs.get(port_id)?;
        let PortDeclSrc::ImplicitAnsiPort(_) = src else {
            return None;
        };

        let name = port_decl_declared_name(&module, port_decl)?;
        port_names.push(name);
        port_items.push((port_decl, src));
    }

    if port_names.is_empty() {
        return None;
    }

    let open_paren = port_list.open_paren()?.text_range_in(port_list.syntax())?;
    let close_paren = port_list.close_paren()?.text_range_in(port_list.syntax())?;
    if !port_list_trigger_range(open_paren, close_paren)?.contains_range(ctx.range()) {
        return None;
    }

    let body_range = module_body_range(ast_module)?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let generated_members = port_items
        .iter()
        .map(|(port_decl, src)| {
            render_ansi_port_declaration(ctx, module_id, port_decl, *src, &text)
        })
        .collect::<Option<Vec<_>>>()?;
    let port_list_replacement = render_port_list(&text, open_paren, close_paren, &port_names)?;
    let body_replacement =
        render_module_body(&text, ast_module, body_range, &generated_members, &[])?;
    let target = port_list.syntax().text_range()?;

    collector.add(ANSI_TO_NON_ANSI_ID, ANSI_TO_NON_ANSI_LABEL, target, |builder| {
        builder.replace(target, port_list_replacement);
        builder.replace(body_range, body_replacement);
    })
}

fn convert_non_ansi_ports_to_ansi(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let ast_module = ctx.find_node_at_offset::<ast::ModuleDeclaration>()?;
    let port_list = ast_module.header().ports()?.as_non_ansi_port_list()?;

    let module_id = ctx.sema().module_to_def(ctx.file_id().into(), ast_module)?;
    let (module, module_src_map) = ctx.sema().db.module_with_source_map(module_id);
    let Ports::NonAnsi { ports, refs, .. } = &module.ports else {
        return None;
    };

    let mut port_names = Vec::new();
    for (_, port) in ports.iter() {
        let mut ref_ids = port.refs.clone()?;
        let ref_id = ref_ids.next()?;
        if ref_ids.next().is_some() {
            return None;
        }

        let port_ref = &refs[ref_id];
        if port_ref.select.is_some() {
            return None;
        }

        let ident = port_ref.ident.as_ref()?;
        if port.label.as_ref() != Some(ident) {
            return None;
        }
        port_names.push(ident.clone());
    }
    if port_names.is_empty() {
        return None;
    }

    let open_paren = port_list.open_paren()?.text_range_in(port_list.syntax())?;
    let close_paren = port_list.close_paren()?.text_range_in(port_list.syntax())?;
    if !port_list_trigger_range(open_paren, close_paren)?.contains_range(ctx.range()) {
        return None;
    }

    let body_range = module_body_range(ast_module)?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let module_scope = ctx.sema().db.module_scope(module_id);
    let port_replacements = port_names
        .iter()
        .map(|name| {
            non_ansi_port_replacement(ctx, &module, &module_src_map, &module_scope, name, &text)
        })
        .collect::<Option<Vec<_>>>()?;
    let ansi_items = port_replacements
        .iter()
        .map(|replacement| replacement.ansi_item.clone())
        .collect::<Vec<_>>();
    let removed_ranges = port_replacements
        .into_iter()
        .flat_map(|replacement| replacement.remove_ranges)
        .collect::<Vec<_>>();
    let port_list_replacement = render_port_list(&text, open_paren, close_paren, &ansi_items)?;
    let body_replacement = render_module_body(&text, ast_module, body_range, &[], &removed_ranges)?;
    let target = port_list.syntax().text_range()?;

    collector.add(NON_ANSI_TO_ANSI_ID, NON_ANSI_TO_ANSI_LABEL, target, |builder| {
        builder.replace(target, port_list_replacement);
        builder.replace(body_range, body_replacement);
    })
}

fn port_list_trigger_range(open: TextRange, close: TextRange) -> Option<TextRange> {
    (open.end() <= close.start()).then(|| TextRange::new(open.end(), close.start()))
}

fn port_decl_declared_name(module: &Module, port_decl: &PortDecl) -> Option<String> {
    let decl_id = single_port_decl_id(port_decl)?;
    Some(module.get(decl_id).name.as_ref()?.to_string())
}

fn single_port_decl_id(port_decl: &PortDecl) -> Option<DeclId> {
    let mut decls = port_decl.decls.clone();
    let decl_id = decls.next()?;
    if decls.next().is_some() {
        return None;
    }
    Some(decl_id)
}

struct NonAnsiPortReplacement {
    ansi_item: String,
    remove_ranges: Vec<TextRange>,
}

fn non_ansi_port_replacement(
    ctx: &CodeActionCtx,
    module: &Module,
    module_src_map: &ModuleSourceMap,
    module_scope: &NameScope,
    name: &Ident,
    text: &str,
) -> Option<NonAnsiPortReplacement> {
    let def = module_scope.lookup(NameContext::Value, name).unique()?;
    let origins = def.origins(ctx.sema().db);

    let port_decl = origins
        .iter()
        .filter_map(|origin| origin.as_decl(ctx.sema().db))
        .find(|decl_id| {
            matches!(module.get(decl_id.value).parent, DeclaratorParent::PortDeclId(_))
        })?
        .value;
    let data_decl = origins
        .iter()
        .filter_map(|origin| origin.as_decl(ctx.sema().db))
        .find(|decl_id| {
            matches!(module.get(decl_id.value).parent, DeclaratorParent::DeclarationId(_))
        })
        .map(|decl_id| decl_id.value);

    let DeclaratorParent::PortDeclId(port_decl_id) = module.get(port_decl).parent else {
        return None;
    };
    let port_decl = module.get(port_decl_id);
    if port_decl_declared_name(module, port_decl).as_deref() != Some(name.as_str()) {
        return None;
    }

    let port_src = module_src_map.port_srcs.get(port_decl_id)?;
    let PortDeclSrc::PortDeclaration(_) = port_src else {
        return None;
    };
    let port_range = port_src.range();

    if let Some(data_decl) = data_decl {
        let data_range = data_decl_range_for_name(module, module_src_map, data_decl, name)?;
        let direction = port_decl.header.dir().display_source(ctx.sema().db).ok()?;
        let data_decl = declaration_text_without_semicolon(text, data_range)?;
        return Some(NonAnsiPortReplacement {
            ansi_item: format!("{direction} {data_decl}"),
            remove_ranges: vec![port_range, data_range],
        });
    }

    Some(NonAnsiPortReplacement {
        ansi_item: declaration_text_without_semicolon(text, port_range)?,
        remove_ranges: vec![port_range],
    })
}

fn data_decl_range_for_name(
    module: &Module,
    module_src_map: &ModuleSourceMap,
    decl_id: DeclId,
    name: &Ident,
) -> Option<TextRange> {
    let decl = module.get(decl_id);
    if decl.name.as_ref() != Some(name) {
        return None;
    }

    let DeclaratorParent::DeclarationId(declaration_id) = decl.parent else {
        return None;
    };
    let declaration = module.get(declaration_id);
    let mut decls = declaration.decls();
    let single_decl_id = decls.next()?;
    if single_decl_id != decl_id || decls.next().is_some() {
        return None;
    }

    let src = module_src_map.declaration_srcs.get(declaration_id)?;
    match src {
        DeclarationSrc::DataDeclaration(_) | DeclarationSrc::NetDeclaration(_) => Some(src.range()),
        _ => None,
    }
}

fn render_ansi_port_declaration(
    ctx: &CodeActionCtx,
    module_id: ModuleId,
    port_decl: &PortDecl,
    src: PortDeclSrc,
    text: &str,
) -> Option<String> {
    let source = text.get(Range::from(src.range()))?;
    if source
        .split_ascii_whitespace()
        .next()
        .is_some_and(|word| matches!(word, "input" | "output" | "inout" | "ref"))
    {
        return Some(format!("{source};"));
    }

    let decl_id = single_port_decl_id(port_decl)?;
    let header = InModule::new(module_id, port_decl.header).display_source(ctx.sema().db).ok()?;
    let decl = InContainer::new(module_id.into(), decl_id).display_signature(ctx.sema().db).ok()?;

    if header.is_empty() { Some(format!("{decl};")) } else { Some(format!("{header} {decl};")) }
}

fn declaration_text_without_semicolon(text: &str, range: TextRange) -> Option<String> {
    Some(text.get(Range::from(range))?.strip_suffix(';')?.to_owned())
}

fn module_body_range(module: ast::ModuleDeclaration<'_>) -> Option<TextRange> {
    let header = module.header();
    Some(TextRange::new(
        header.semi()?.text_range_in(header.syntax())?.end(),
        module.endmodule()?.text_range_in(module.syntax())?.start(),
    ))
}

fn render_port_list(
    text: &str,
    open: TextRange,
    close: TextRange,
    items: &[String],
) -> Option<String> {
    let content = text.get(usize::from(open.end())..usize::from(close.start()))?;
    if content.contains('\n') {
        let close_indent = line_indent(text, close.start());
        let item_indent = format!("{close_indent}    ");
        let rendered = items
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let suffix = if idx + 1 == items.len() { "" } else { "," };
                format!("{item_indent}{item}{suffix}")
            })
            .collect::<Vec<_>>()
            .join("\n");
        Some(format!("(\n{rendered}\n{close_indent})"))
    } else {
        Some(format!("({})", items.join(", ")))
    }
}

fn render_module_body(
    text: &str,
    module: ast::ModuleDeclaration<'_>,
    body_range: TextRange,
    prefix_items: &[String],
    remove_ranges: &[TextRange],
) -> Option<String> {
    let mut items = prefix_items.to_vec();
    let mut body = text.get(Range::from(body_range))?.to_owned();
    remove_ranges_from_body(&mut body, body_range, remove_ranges)?;
    let body = body.trim();
    if !body.is_empty() {
        items.push(body.to_owned());
    }

    let endmodule = module.endmodule()?.text_range_in(module.syntax())?;
    let module_indent = line_indent(text, endmodule.start());
    if items.is_empty() {
        return Some(format!("\n{module_indent}"));
    }

    let item_indent = format!("{module_indent}    ");
    let rendered = items
        .into_iter()
        .map(|item| indent_block(&item, &item_indent))
        .collect::<Vec<_>>()
        .join("\n");
    Some(format!("\n{rendered}\n{module_indent}"))
}

fn remove_ranges_from_body(
    body: &mut String,
    body_range: TextRange,
    remove_ranges: &[TextRange],
) -> Option<()> {
    let body_start = usize::from(body_range.start());
    let body_end = usize::from(body_range.end());
    let mut ranges = remove_ranges
        .iter()
        .filter(|range| body_range.contains_range(**range))
        .map(|range| {
            Some((
                usize::from(range.start()).checked_sub(body_start)?,
                usize::from(range.end()).checked_sub(body_start)?,
            ))
        })
        .collect::<Option<Vec<_>>>()?;

    ranges.sort_by_key(|(start, _)| *start);
    for (start, end) in ranges.into_iter().rev() {
        if start > end || body_start + end > body_end {
            return None;
        }
        body.replace_range(start..end, "");
    }
    Some(())
}

fn indent_block(text: &str, indent: &str) -> String {
    text.lines().map(|line| format!("{indent}{line}")).join("\n")
}
