use ide_db::root_db::RootDb;
use span::FilePosition;

use super::{
    CompletionItem, expr, keywords, member, named, paren_list, port_list, preproc, sensitivity_list,
};
use crate::completion::context::{
    CompletionContext, ExpectedSyntax, HashKind, LexContext, ParenListKind, PortListKind,
    TriggerChar,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CompletionPlan {
    None,
    Directives,
    Keywords,
    Expression,
    PortConnectionName,
    ParameterAssignmentName,
    MemberName,
    PortConnectionExpr,
    ParameterAssignmentExpr,
    AfterParamValueAssignmentHash,
    AfterParameterPortListHash,
    ParamValueAssignment,
    ParameterPortList,
    PortConnections,
    Arguments,
    AnsiPorts,
    FunctionPorts,
    NonAnsiPorts,
    EventControl,
}

impl CompletionPlan {
    pub(super) fn from_context(ctx: &CompletionContext) -> Self {
        if matches!(
            ctx.expectation.map(|expectation| expectation.syntax),
            Some(ExpectedSyntax::DirectiveName)
        ) {
            return Self::Directives;
        }

        if ctx.lex != LexContext::Code {
            return Self::None;
        }

        let Some(expectation) = ctx.expectation else {
            return Self::None;
        };

        if newline_trigger_outside_port_list(ctx) {
            return Self::None;
        }

        if punctuation_trigger_without_specific_expectation(ctx) {
            return Self::None;
        }

        match expectation.syntax {
            ExpectedSyntax::DirectiveName | ExpectedSyntax::DeclName => Self::None,
            ExpectedSyntax::CompilationUnitItem
            | ExpectedSyntax::ModuleHeaderItem
            | ExpectedSyntax::ModuleItem
            | ExpectedSyntax::GenerateItem
            | ExpectedSyntax::SpecifyItem
            | ExpectedSyntax::ConfigItem { .. }
            | ExpectedSyntax::BlockItem { .. }
            | ExpectedSyntax::Statement => Self::Keywords,
            ExpectedSyntax::Expression => Self::Expression,
            ExpectedSyntax::PortConnectionName => Self::PortConnectionName,
            ExpectedSyntax::ParameterAssignmentName => Self::ParameterAssignmentName,
            ExpectedSyntax::MemberName => Self::MemberName,
            ExpectedSyntax::PortConnectionExpr => Self::PortConnectionExpr,
            ExpectedSyntax::ParameterAssignmentExpr => Self::ParameterAssignmentExpr,
            ExpectedSyntax::AfterParamValueAssignmentHash => Self::AfterParamValueAssignmentHash,
            ExpectedSyntax::AfterParameterPortListHash => Self::AfterParameterPortListHash,
            ExpectedSyntax::ParamValueAssignment => Self::ParamValueAssignment,
            ExpectedSyntax::ParameterPortListItem => Self::ParameterPortList,
            ExpectedSyntax::PortConnection => Self::PortConnections,
            ExpectedSyntax::ArgumentExpr => Self::Arguments,
            ExpectedSyntax::AnsiPortItem => Self::AnsiPorts,
            ExpectedSyntax::FunctionPortItem => Self::FunctionPorts,
            ExpectedSyntax::NonAnsiPortName => Self::NonAnsiPorts,
            ExpectedSyntax::EventControl { .. } => Self::EventControl,
        }
    }

    pub(super) fn complete(
        self,
        db: &RootDb,
        position: FilePosition,
        ctx: &CompletionContext,
    ) -> Vec<CompletionItem> {
        match self {
            Self::None => Vec::new(),
            Self::Directives => preproc::complete_directives(ctx),
            Self::Keywords => keywords::complete_keywords(db, position, &ctx.prefix, ctx),
            Self::Expression => expr::complete_expression(db, position, &ctx.prefix, ctx),
            Self::PortConnectionName => {
                named::complete_named_port_names(db, position, &ctx.prefix, ctx)
            }
            Self::ParameterAssignmentName => {
                named::complete_named_param_names(db, position, &ctx.prefix, ctx)
            }
            Self::MemberName => member::complete_member_access(db, position, &ctx.prefix, ctx),
            Self::PortConnectionExpr => {
                named::complete_named_port_conn_expr(db, position, &ctx.prefix, ctx)
            }
            Self::ParameterAssignmentExpr => {
                named::complete_named_param_assign_expr(db, position, &ctx.prefix, ctx)
            }
            Self::AfterParamValueAssignmentHash => {
                paren_list::complete_after_hash(&ctx.prefix, ctx, HashKind::ParamValueAssignment)
            }
            Self::AfterParameterPortListHash => {
                paren_list::complete_after_hash(&ctx.prefix, ctx, HashKind::ParameterPortList)
            }
            Self::ParamValueAssignment => paren_list::complete_in_paren_list(
                db,
                position,
                &ctx.prefix,
                ctx,
                ParenListKind::ParamValueAssignment,
            ),
            Self::ParameterPortList => paren_list::complete_in_paren_list(
                db,
                position,
                &ctx.prefix,
                ctx,
                ParenListKind::ParameterPortList,
            ),
            Self::PortConnections => paren_list::complete_in_paren_list(
                db,
                position,
                &ctx.prefix,
                ctx,
                ParenListKind::PortConnections,
            ),
            Self::Arguments => paren_list::complete_in_paren_list(
                db,
                position,
                &ctx.prefix,
                ctx,
                ParenListKind::Arguments,
            ),
            Self::AnsiPorts => {
                port_list::complete_in_port_list(db, position, &ctx.prefix, ctx, PortListKind::Ansi)
            }
            Self::FunctionPorts => port_list::complete_in_port_list(
                db,
                position,
                &ctx.prefix,
                ctx,
                PortListKind::Function,
            ),
            Self::NonAnsiPorts => port_list::complete_in_port_list(
                db,
                position,
                &ctx.prefix,
                ctx,
                PortListKind::NonAnsi,
            ),
            Self::EventControl => {
                sensitivity_list::complete_sensitivity_list(db, position, &ctx.prefix, ctx)
            }
        }
    }
}

fn newline_trigger_outside_port_list(ctx: &CompletionContext) -> bool {
    ctx.trigger == Some(TriggerChar::Newline)
        && ctx.expectation.is_none_or(|expectation| !expectation.syntax.accepts_newline_trigger())
}

fn punctuation_trigger_without_specific_expectation(ctx: &CompletionContext) -> bool {
    ctx.trigger.is_some()
        && ctx.expectation.is_some_and(|expectation| {
            expectation.syntax.is_punctuation_trigger_suppressed_context()
        })
        && ctx.prefix.is_empty()
        && ctx.replacement.is_empty()
}
