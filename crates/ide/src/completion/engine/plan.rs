use ide_db::root_db::RootDb;
use span::FilePosition;

use super::{
    CompletionItem, candidate, expr, keywords, member, named, paren_list, port_list, preproc,
    sensitivity_list,
};
use crate::completion::{context::CompletionContext, request::CompletionRequest};

pub(super) fn complete_request(
    db: &RootDb,
    position: FilePosition,
    ctx: &CompletionContext,
    request: CompletionRequest,
) -> Vec<CompletionItem> {
    let candidates = match request {
        CompletionRequest::Directives => preproc::complete_directives(ctx),
        CompletionRequest::Keywords(expected) => {
            keywords::complete_keywords(db, position, &ctx.prefix, ctx, expected)
        }
        CompletionRequest::Expression => expr::complete_expression(db, position, &ctx.prefix, ctx),
        CompletionRequest::PortConnectionName => {
            named::complete_named_port_names(db, position, &ctx.prefix, ctx)
        }
        CompletionRequest::ParameterAssignmentName => {
            named::complete_named_param_names(db, position, &ctx.prefix, ctx)
        }
        CompletionRequest::MemberName => {
            member::complete_member_access(db, position, &ctx.prefix, ctx)
        }
        CompletionRequest::PortConnectionExpr => {
            named::complete_named_port_conn_expr(db, position, &ctx.prefix, ctx)
        }
        CompletionRequest::ParameterAssignmentExpr => {
            named::complete_named_param_assign_expr(db, position, &ctx.prefix, ctx)
        }
        CompletionRequest::AfterHash(kind) => {
            paren_list::complete_after_hash(&ctx.prefix, ctx, kind)
        }
        CompletionRequest::ParenList(kind) => {
            paren_list::complete_in_paren_list(db, position, &ctx.prefix, ctx, kind)
        }
        CompletionRequest::PortList(kind) => {
            port_list::complete_in_port_list(db, position, &ctx.prefix, ctx, kind)
        }
        CompletionRequest::EventControl { wrap_in_parens } => {
            sensitivity_list::complete_sensitivity_list(
                db,
                position,
                &ctx.prefix,
                ctx,
                wrap_in_parens,
            )
        }
    };

    candidate::finalize_candidates(candidates, &ctx.prefix)
}
