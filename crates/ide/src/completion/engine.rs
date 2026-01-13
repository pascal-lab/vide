mod instantiation;
mod keywords;
mod member;
mod named;
mod paren_list;
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
    CompletionContext, DotKind, LexContext, Qualifier, SynContext, TriggerChar, completion_context,
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
    if ctx.lex == LexContext::PreprocDirective {
        return preproc::complete_directives(&ctx.prefix, ctx);
    }

    if ctx.lex != LexContext::Code {
        return Vec::new();
    }

    if ctx.syn == SynContext::SensitivityList {
        return sensitivity_list::complete_sensitivity_list(db, position, &ctx.prefix, ctx);
    }

    match ctx.qualifier {
        Some(Qualifier::AfterDot(after_dot)) => match after_dot.kind {
            DotKind::NamedPort => named::complete_named_port_names(db, position, &ctx.prefix, ctx),
            DotKind::NamedParam => {
                named::complete_named_param_names(db, position, &ctx.prefix, ctx)
            }
            DotKind::Member => member::complete_member_access(db, position, &ctx.prefix, ctx),
        },
        Some(Qualifier::InNamedPortConnExpr) => {
            named::complete_named_port_conn_expr(db, position, &ctx.prefix, ctx)
        }
        Some(Qualifier::InNamedParamAssignExpr) => {
            named::complete_named_param_assign_expr(db, position, &ctx.prefix, ctx)
        }
        None => keywords::complete_keywords(db, position, &ctx.prefix, ctx),
        Some(Qualifier::AfterHash(after_hash)) => {
            paren_list::complete_after_hash(&ctx.prefix, ctx, after_hash.kind)
        }
        Some(Qualifier::InParenList(in_parens)) => {
            paren_list::complete_in_paren_list(db, position, &ctx.prefix, ctx, in_parens.kind)
        }
        Some(Qualifier::AfterAt(_)) => Vec::new(),
        Some(Qualifier::AfterBacktick) => preproc::complete_directives(&ctx.prefix, ctx),
    }
}
