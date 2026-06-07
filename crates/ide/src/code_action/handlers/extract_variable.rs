use std::ops::Range;

use hir::{
    base_db::source_db::SourceDb,
    container::InContainer,
    display::HirDisplay,
    type_infer::{BuiltinTy, Ty, type_of_expr, type_of_path_resolution},
};
use syntax::{
    SyntaxAncestors, SyntaxKind, TokenKind, WalkEvent,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};
use utils::{
    get::GetRef,
    text_edit::{TextRange, TextSize},
};

use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, line_indent,
};

const ID: CodeActionId =
    CodeActionId { name: "extract_variable", kind: CodeActionKind::RefactorExtract, repair: None };

pub(super) fn extract_variable(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let text = ctx.sema().db.file_text(ctx.file_id());
    let expr = selected_expression(ctx, &text)?;
    let expr_range = expr.syntax().text_range()?;
    let target = extract_target(&text, expr)?;
    let expr_text = text.get(Range::from(expr_range))?.trim().to_owned();
    let name = fresh_variable_name(&text, "value");

    collector.add(ID, "Extract into variable", expr_range, |builder| {
        let ty_text = extracted_variable_type(ctx, expr).unwrap_or_else(|| "logic".to_owned());
        let declaration = target.declaration(&ty_text, &name, &expr_text);
        builder.insert(target.insert_offset, declaration);
        builder.replace(expr_range, name);
    })
}

struct ExtractTarget {
    insert_offset: TextSize,
    indent: String,
    declaration_style: DeclarationStyle,
}

impl ExtractTarget {
    fn declaration(&self, ty_text: &str, name: &str, expr_text: &str) -> String {
        match self.declaration_style {
            DeclarationStyle::Local => {
                format!("{}{ty_text} {name} = {expr_text};\n", self.indent)
            }
            DeclarationStyle::ContinuousNet => {
                format!("{}wire {ty_text} {name} = {expr_text};\n", self.indent)
            }
        }
    }
}

enum DeclarationStyle {
    Local,
    ContinuousNet,
}

fn extract_target(text: &str, expr: ast::Expression<'_>) -> Option<ExtractTarget> {
    if let Some(stmt) =
        SyntaxAncestors::start_from(expr.syntax()).find_map(ast::ExpressionStatement::cast)
        && stmt.syntax().parent().and_then(ast::BlockStatement::cast).is_some()
    {
        let stmt_range = stmt.syntax().text_range()?;
        return Some(ExtractTarget {
            insert_offset: stmt_range.start(),
            indent: line_indent(text, stmt_range.start()),
            declaration_style: DeclarationStyle::Local,
        });
    }

    let assign = SyntaxAncestors::start_from(expr.syntax()).find_map(ast::ContinuousAssign::cast)?;
    expression_is_assignment_rhs(expr)?;
    let assign_range = assign.syntax().text_range()?;
    Some(ExtractTarget {
        insert_offset: assign_range.start(),
        indent: line_indent(text, assign_range.start()),
        declaration_style: DeclarationStyle::ContinuousNet,
    })
}

fn expression_is_assignment_rhs(expr: ast::Expression<'_>) -> Option<()> {
    assignment_expression_containing_rhs(expr)
        .filter(|binary| binary.operator_token().is_some_and(|token| token.kind() == TokenKind::EQUALS))
        .map(|_| ())
}

fn assignment_expression_containing_rhs(
    expr: ast::Expression<'_>,
) -> Option<ast::BinaryExpression<'_>> {
    let expr_range = expr.syntax().text_range()?;
    SyntaxAncestors::start_from(expr.syntax()).filter_map(ast::BinaryExpression::cast).find(
        |binary| {
            is_assignment_expression(binary.syntax().kind())
                && binary
                    .right()
                    .syntax()
                    .text_range()
                    .is_some_and(|range| range.contains_range(expr_range))
        },
    )
}

fn is_assignment_expression(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::ASSIGNMENT_EXPRESSION
            | SyntaxKind::NONBLOCKING_ASSIGNMENT_EXPRESSION
            | SyntaxKind::ADD_ASSIGNMENT_EXPRESSION
            | SyntaxKind::SUBTRACT_ASSIGNMENT_EXPRESSION
            | SyntaxKind::MULTIPLY_ASSIGNMENT_EXPRESSION
            | SyntaxKind::DIVIDE_ASSIGNMENT_EXPRESSION
            | SyntaxKind::MOD_ASSIGNMENT_EXPRESSION
            | SyntaxKind::AND_ASSIGNMENT_EXPRESSION
            | SyntaxKind::OR_ASSIGNMENT_EXPRESSION
            | SyntaxKind::XOR_ASSIGNMENT_EXPRESSION
            | SyntaxKind::LOGICAL_LEFT_SHIFT_ASSIGNMENT_EXPRESSION
            | SyntaxKind::LOGICAL_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION
            | SyntaxKind::ARITHMETIC_LEFT_SHIFT_ASSIGNMENT_EXPRESSION
            | SyntaxKind::ARITHMETIC_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION
    )
}

fn selected_expression<'a>(ctx: &'a CodeActionCtx<'_>, text: &str) -> Option<ast::Expression<'a>> {
    let range = trim_range(text, ctx.range())?;
    if range.is_empty() {
        return None;
    }

    ctx.syntax().node_preorder().find_map(|event| match event {
        WalkEvent::Enter(node) => {
            let expr = ast::Expression::cast(node)?;
            (expr.syntax().text_range()? == range).then_some(expr)
        }
        WalkEvent::Leave(_) => None,
    })
}

fn trim_range(text: &str, range: TextRange) -> Option<TextRange> {
    let selected = text.get(Range::<usize>::from(range))?;
    let trimmed_start = selected.trim_start();
    let trimmed = trimmed_start.trim_end();

    let leading = selected.len() - trimmed_start.len();
    let trailing = trimmed_start.len() - trimmed.len();
    Some(TextRange::new(
        range.start() + TextSize::from(leading as u32),
        range.end() - TextSize::from(trailing as u32),
    ))
}

fn extracted_variable_type(ctx: &CodeActionCtx<'_>, expr: ast::Expression<'_>) -> Option<String> {
    let ty = type_of_expr(ctx.sema().db, ctx.sema().resolve_expr(ctx.file_id().into(), expr)?).ty;
    render_ty(ctx, &ty).or_else(|| {
        expected_type_for_assignment_rhs(ctx, expr).and_then(|ty| render_ty(ctx, &ty))
    })
}

fn expected_type_for_assignment_rhs(
    ctx: &CodeActionCtx<'_>,
    expr: ast::Expression<'_>,
) -> Option<Ty> {
    let assignment = assignment_expression_containing_rhs(expr)?;
    let res = ctx
        .sema()
        .expr_to_def(ctx.sema().resolve_expr(ctx.file_id().into(), assignment.left())?)?;
    Some(type_of_path_resolution(ctx.sema().db, res).ty)
}

fn render_ty(ctx: &CodeActionCtx<'_>, ty: &Ty) -> Option<String> {
    match ty {
        Ty::Builtin(BuiltinTy::Data { id, container }) => {
            InContainer::new(*container, hir::hir_def::expr::data_ty::DataTy::Builtin(*id))
                .display_source(ctx.sema().db)
                .ok()
        }
        Ty::Alias { typedef, .. } => {
            let container = typedef.cont_id.to_container(ctx.sema().db);
            container.get(typedef.value).name.as_ref().map(ToString::to_string)
        }
        Ty::Struct(struct_ref) => {
            let container = struct_ref.cont_id.to_container(ctx.sema().db);
            container.get(struct_ref.value).name.as_ref().map(ToString::to_string)
        }
        Ty::Unknown
        | Ty::Error
        | Ty::Void
        | Ty::Module(_)
        | Ty::GenerateBlock(_)
        | Ty::Block(_) => None,
    }
}

fn fresh_variable_name(text: &str, base: &str) -> String {
    if !text.contains(base) {
        return base.to_owned();
    }

    let mut idx = 1usize;
    loop {
        let candidate = format!("{base}_{idx}");
        if !text.contains(&candidate) {
            return candidate;
        }
        idx += 1;
    }
}
