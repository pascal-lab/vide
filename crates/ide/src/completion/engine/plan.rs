use ide_db::root_db::RootDb;
use span::FilePosition;

use super::{
    CompletionItem, candidate, expr, keywords, member, named, paren_list, port_list, preproc,
    sensitivity_list, system,
};
use crate::completion::{
    context::CompletionContext,
    request::{CompletionProvider, CompletionRequest},
};

pub(super) fn complete_request(
    db: &RootDb,
    position: FilePosition,
    ctx: &CompletionContext,
    request: CompletionRequest,
) -> Vec<CompletionItem> {
    let candidates =
        request.providers().flat_map(|provider| complete_provider(db, position, ctx, provider));

    candidate::finalize_candidates(candidates, &ctx.prefix)
}

fn complete_provider(
    db: &RootDb,
    position: FilePosition,
    ctx: &CompletionContext,
    provider: CompletionProvider,
) -> Vec<candidate::CompletionCandidate> {
    match provider {
        CompletionProvider::Directives => preproc::complete_directives(ctx),
        CompletionProvider::Keywords(provider) => {
            keywords::complete_keywords(db, position, &ctx.prefix, ctx, provider)
        }
        CompletionProvider::SystemTasks => system::complete_system_tasks(&ctx.prefix, ctx),
        CompletionProvider::Expression => expr::complete_expression(db, position, &ctx.prefix, ctx),
        CompletionProvider::PortConnectionName => {
            named::complete_named_port_names(db, position, &ctx.prefix, ctx)
        }
        CompletionProvider::ParameterAssignmentName => {
            named::complete_named_param_names(db, position, &ctx.prefix, ctx)
        }
        CompletionProvider::MemberName => {
            member::complete_member_access(db, position, &ctx.prefix, ctx)
        }
        CompletionProvider::PortConnectionExpr => {
            named::complete_named_port_conn_expr(db, position, &ctx.prefix, ctx)
        }
        CompletionProvider::ParameterAssignmentExpr => {
            named::complete_named_param_assign_expr(db, position, &ctx.prefix, ctx)
        }
        CompletionProvider::ElseClause => keywords::complete_else_clause(&ctx.prefix, ctx),
        CompletionProvider::AfterHash(kind) => {
            paren_list::complete_after_hash(&ctx.prefix, ctx, kind)
        }
        CompletionProvider::ParenList(kind) => {
            paren_list::complete_in_paren_list(db, position, &ctx.prefix, ctx, kind)
        }
        CompletionProvider::PortList(kind) => {
            port_list::complete_in_port_list(db, position, &ctx.prefix, ctx, kind)
        }
        CompletionProvider::EventControl { wrap_in_parens } => {
            sensitivity_list::complete_sensitivity_list(
                db,
                position,
                &ctx.prefix,
                ctx,
                wrap_in_parens,
            )
        }
    }
}
