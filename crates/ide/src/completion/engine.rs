mod expr;
mod instantiation;
mod keywords;
mod member;
mod named;
mod paren_list;
mod port_list;
mod preproc;
mod sensitivity_list;
mod snippets;
mod typed_filter;

#[cfg(test)]
mod tests;

use ide_db::root_db::RootDb;
use span::FilePosition;

pub use self::named::{CompletionItem, CompletionItemKind};
use crate::completion::context::{
    CompletionContext, ExpectedSyntax, HashKind, LexContext, ParenListKind, PortListKind,
    TriggerChar, completion_context,
};

pub fn completions(
    db: &RootDb,
    position: FilePosition,
    trigger: Option<TriggerChar>,
) -> Vec<CompletionItem> {
    let ctx = completion_context(db, position, trigger);
    completions_with_context(db, position, &ctx)
}

fn completions_with_context(
    db: &RootDb,
    position: FilePosition,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    if matches!(
        ctx.expectation.map(|expectation| expectation.syntax),
        Some(ExpectedSyntax::DirectiveName)
    ) {
        return preproc::complete_directives(ctx);
    }

    if ctx.lex != LexContext::Code {
        return Vec::new();
    }

    let Some(expectation) = ctx.expectation else {
        return Vec::new();
    };

    if newline_trigger_outside_port_list(ctx) {
        return Vec::new();
    }

    if punctuation_trigger_without_specific_expectation(ctx) {
        return Vec::new();
    }

    match expectation.syntax {
        ExpectedSyntax::DirectiveName | ExpectedSyntax::DeclName => Vec::new(),
        ExpectedSyntax::CompilationUnitItem
        | ExpectedSyntax::ModuleHeaderItem
        | ExpectedSyntax::ModuleItem
        | ExpectedSyntax::BlockItem { .. }
        | ExpectedSyntax::Statement => keywords::complete_keywords(db, position, &ctx.prefix, ctx),
        ExpectedSyntax::Expression => expr::complete_expression(db, position, &ctx.prefix, ctx),
        ExpectedSyntax::PortConnectionName => {
            named::complete_named_port_names(db, position, &ctx.prefix, ctx)
        }
        ExpectedSyntax::ParameterAssignmentName => {
            named::complete_named_param_names(db, position, &ctx.prefix, ctx)
        }
        ExpectedSyntax::MemberName => {
            member::complete_member_access(db, position, &ctx.prefix, ctx)
        }
        ExpectedSyntax::PortConnectionExpr => {
            named::complete_named_port_conn_expr(db, position, &ctx.prefix, ctx)
        }
        ExpectedSyntax::ParameterAssignmentExpr => {
            named::complete_named_param_assign_expr(db, position, &ctx.prefix, ctx)
        }
        ExpectedSyntax::AfterParamValueAssignmentHash => {
            paren_list::complete_after_hash(&ctx.prefix, ctx, HashKind::ParamValueAssignment)
        }
        ExpectedSyntax::AfterParameterPortListHash => {
            paren_list::complete_after_hash(&ctx.prefix, ctx, HashKind::ParameterPortList)
        }
        ExpectedSyntax::ParamValueAssignment => paren_list::complete_in_paren_list(
            db,
            position,
            &ctx.prefix,
            ctx,
            ParenListKind::ParamValueAssignment,
        ),
        ExpectedSyntax::ParameterPortListItem => paren_list::complete_in_paren_list(
            db,
            position,
            &ctx.prefix,
            ctx,
            ParenListKind::ParameterPortList,
        ),
        ExpectedSyntax::PortConnection => paren_list::complete_in_paren_list(
            db,
            position,
            &ctx.prefix,
            ctx,
            ParenListKind::PortConnections,
        ),
        ExpectedSyntax::ArgumentExpr => paren_list::complete_in_paren_list(
            db,
            position,
            &ctx.prefix,
            ctx,
            ParenListKind::Arguments,
        ),
        ExpectedSyntax::AnsiPortItem => {
            port_list::complete_in_port_list(db, position, &ctx.prefix, ctx, PortListKind::Ansi)
        }
        ExpectedSyntax::FunctionPortItem => {
            port_list::complete_in_port_list(db, position, &ctx.prefix, ctx, PortListKind::Function)
        }
        ExpectedSyntax::NonAnsiPortName => {
            port_list::complete_in_port_list(db, position, &ctx.prefix, ctx, PortListKind::NonAnsi)
        }
        ExpectedSyntax::EventControl { .. } => {
            sensitivity_list::complete_sensitivity_list(db, position, &ctx.prefix, ctx)
        }
    }
}

fn newline_trigger_outside_port_list(ctx: &CompletionContext) -> bool {
    ctx.trigger == Some(TriggerChar::Newline)
        && ctx.expectation.is_none_or(|expectation| {
            !matches!(
                expectation.syntax,
                ExpectedSyntax::AnsiPortItem | ExpectedSyntax::FunctionPortItem
            )
        })
}

fn punctuation_trigger_without_specific_expectation(ctx: &CompletionContext) -> bool {
    ctx.trigger.is_some()
        && ctx.expectation.is_some_and(|expectation| {
            matches!(
                expectation.syntax,
                ExpectedSyntax::CompilationUnitItem
                    | ExpectedSyntax::ModuleHeaderItem
                    | ExpectedSyntax::ModuleItem
                    | ExpectedSyntax::BlockItem { .. }
                    | ExpectedSyntax::Statement
            )
        })
        && ctx.prefix.is_empty()
        && ctx.replacement.is_empty()
}
