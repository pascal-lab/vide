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
    CompletionContext, CompletionSite, HashKind, LexContext, ParenListKind, PortListKind,
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
    if ctx.site == CompletionSite::PreprocDirective {
        return preproc::complete_directives(ctx);
    }

    if ctx.lex != LexContext::Code || ctx.site == CompletionSite::Forbidden {
        return Vec::new();
    }

    if punctuation_trigger_without_site(ctx) {
        return Vec::new();
    }

    match ctx.site {
        CompletionSite::Forbidden | CompletionSite::PreprocDirective => Vec::new(),
        CompletionSite::TopLevel
        | CompletionSite::ModuleHeader
        | CompletionSite::ModuleItemStart => {
            keywords::complete_keywords(db, position, &ctx.prefix, ctx)
        }
        CompletionSite::Expr => expr::complete_expression(db, position, &ctx.prefix, ctx),
        CompletionSite::NamedPortName => {
            named::complete_named_port_names(db, position, &ctx.prefix, ctx)
        }
        CompletionSite::NamedParamName => {
            named::complete_named_param_names(db, position, &ctx.prefix, ctx)
        }
        CompletionSite::MemberAccess => {
            member::complete_member_access(db, position, &ctx.prefix, ctx)
        }
        CompletionSite::NamedPortConnExpr => {
            named::complete_named_port_conn_expr(db, position, &ctx.prefix, ctx)
        }
        CompletionSite::NamedParamAssignExpr => {
            named::complete_named_param_assign_expr(db, position, &ctx.prefix, ctx)
        }
        CompletionSite::AfterParamValueAssignmentHash => {
            paren_list::complete_after_hash(&ctx.prefix, ctx, HashKind::ParamValueAssignment)
        }
        CompletionSite::AfterParameterPortListHash => {
            paren_list::complete_after_hash(&ctx.prefix, ctx, HashKind::ParameterPortList)
        }
        CompletionSite::ParamValueAssignment => paren_list::complete_in_paren_list(
            db,
            position,
            &ctx.prefix,
            ctx,
            ParenListKind::ParamValueAssignment,
        ),
        CompletionSite::ParameterPortList => paren_list::complete_in_paren_list(
            db,
            position,
            &ctx.prefix,
            ctx,
            ParenListKind::ParameterPortList,
        ),
        CompletionSite::PortConnections => paren_list::complete_in_paren_list(
            db,
            position,
            &ctx.prefix,
            ctx,
            ParenListKind::PortConnections,
        ),
        CompletionSite::Arguments => paren_list::complete_in_paren_list(
            db,
            position,
            &ctx.prefix,
            ctx,
            ParenListKind::Arguments,
        ),
        CompletionSite::AnsiPortList => {
            port_list::complete_in_port_list(db, position, &ctx.prefix, ctx, PortListKind::Ansi)
        }
        CompletionSite::NonAnsiPortList => {
            port_list::complete_in_port_list(db, position, &ctx.prefix, ctx, PortListKind::NonAnsi)
        }
        CompletionSite::AfterAtEventControl | CompletionSite::SensitivityList => {
            sensitivity_list::complete_sensitivity_list(db, position, &ctx.prefix, ctx)
        }
    }
}

fn punctuation_trigger_without_site(ctx: &CompletionContext) -> bool {
    ctx.trigger.is_some()
        && matches!(
            ctx.site,
            CompletionSite::TopLevel
                | CompletionSite::ModuleHeader
                | CompletionSite::ModuleItemStart
        )
        && ctx.prefix.is_empty()
        && ctx.replacement.is_empty()
}
