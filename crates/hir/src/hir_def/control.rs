use crate::hir_def::{
    expr::{ExprId, LowerExpr, MinTypMaxExpr, Path},
    try_match, InFile, SourceMap,
};
use la_arena::{Arena, Idx};
use syntax::ast::{self, ptr};

use super::literal::Literal;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum DelayControl {
    Val(Literal),
    MinTypMax(MinTypMaxExpr),
}

pub(crate) trait LowerDelayControl: LowerExpr {
    fn lower_delay_control(&mut self, delay_control: &ast::DelayControl) -> Option<DelayControl> {
        try_match! {
            delay_control.delay_value(), delay_value => {
                Some(DelayControl::Val(self.lower_delay_value(&delay_value)?))
            },
            delay_control.mintypmax_expression(), mintypmax_expression => {
                Some(DelayControl::MinTypMax(self.lower_min_typ_max_expr(&mintypmax_expression)))
            },
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum EventExpr {
    Expr {
        sensitivity: Option<Sensitivity>,
        expr: ExprId,
        // TODO: iff expression
    },
    Or(EventExprId, EventExprId),
    // TODO: sequence_instance
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Sensitivity {
    Posedge,
    Negedge,
    Edge,
}

pub(crate) fn lower_sensitivity(edge_identifier: &ast::EdgeIdentifier) -> Option<Sensitivity> {
    try_match! {
        edge_identifier.token_posedge(), _ => Some(Sensitivity::Posedge),
        edge_identifier.token_negedge(), _ => Some(Sensitivity::Negedge),
        edge_identifier.token_edge(), _ => Some(Sensitivity::Edge),
        _ => None,
    }
}

pub type EventExprId = Idx<EventExpr>;

pub(crate) trait LowerEventExpr: LowerExpr {
    fn arena_event_exprs(&mut self) -> &mut Arena<EventExpr>;

    fn src_map_event_expr(&mut self) -> &mut SourceMap<InFile<ptr::EventExpressionPtr>, EventExpr>;

    fn lower_event_expr(&mut self, event_expr: &ast::EventExpression) -> Option<EventExprId> {
        try_match! {
            // FIXME: support for "iff expression" and "(event_identifier)" is needed
            event_expr.expression(), expr => {
                let sensitivity = event_expr.edge_identifier().and_then(|edge_identifier| lower_sensitivity(&edge_identifier));
                let expr = self.lower_expr(&expr);
                let src = self.in_file(event_expr.to_ptr());
                let idx = self.arena_event_exprs().alloc(EventExpr::Expr{sensitivity, expr});
                self.src_map_event_expr().insert(src, idx);
                Some(idx)
            },
            event_expr.token_or(), _ => {
                let lhs = self.lower_event_expr(&event_expr.event_expressions().next()?)?;
                let rhs = self.lower_event_expr(&event_expr.event_expressions().next()?)?;
                let src = self.in_file(event_expr.to_ptr());
                let idx = self.arena_event_exprs().alloc(EventExpr::Or(lhs, rhs));
                self.src_map_event_expr().insert(src, idx);
                Some(idx)
            },
            event_expr.token_comma(), _ => {
                let sensitivity = lower_sensitivity(&event_expr.edge_identifier()?);
                let expr = self.lower_expr(&event_expr.expression()?);
                let src = self.in_file(event_expr.to_ptr());
                let idx = self.arena_event_exprs().alloc(EventExpr::Expr{sensitivity, expr});
                self.src_map_event_expr().insert(src, idx);
                Some(idx)
            },
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum EventControl {
    Path(Path),
    EventExpr(Idx<EventExpr>),
    Star,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum DelayOrEventControl {
    DelayControl(DelayControl),
    EventControl(EventControl),
    RepeatControl { expr: ExprId, event_control: EventControl },
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum ProceduralTimingControlControl {
    DelayControl(DelayControl),
    EventControl(EventControl),
    // TODO: cycle_delay
}

pub(crate) trait LowerTimingControl: LowerDelayControl + LowerEventExpr {
    fn lower_delay_or_event_control(
        &mut self,
        control: &ast::DelayOrEventControl,
    ) -> Option<DelayOrEventControl> {
        try_match! {
            control.delay_control(), delay_control => {
                let delay_control = self.lower_delay_control(&delay_control)?;
                Some(DelayOrEventControl::DelayControl(delay_control))
            },
            control.event_control(), event_control => {
                let event_control = self.lower_event_control(&event_control)?;
                Some(DelayOrEventControl::EventControl(event_control))
            },
            control.token_repeat(), _ => {
                let expr = self.lower_expr(&control.expression()?);
                let event_control = self.lower_event_control(&control.event_control()?)?;
                Some(DelayOrEventControl::RepeatControl{expr, event_control})
            },
            _ => None,
        }
    }

    fn lower_procedural_timing_control(
        &mut self,
        control: &ast::ProceduralTimingControl,
    ) -> Option<ProceduralTimingControlControl> {
        try_match! {
            control.delay_control(), delay_control => {
                let delay_control = self.lower_delay_control(&delay_control)?;
                Some(ProceduralTimingControlControl::DelayControl(delay_control))
            },
            control.event_control(), event_control => {
                let event_control = self.lower_event_control(&event_control)?;
                Some(ProceduralTimingControlControl::EventControl(event_control))
            },
            control.cycle_delay(), _cycle_delay => {
                unimplemented!("cycle_delay")
            },
            _ => None,
        }
    }

    fn lower_event_control(&mut self, event_control: &ast::EventControl) -> Option<EventControl> {
        try_match! {
            event_control.hierarchical_event_identifier(), _path => {
                // let path = self.lower_path(&path)?;
                // Some(EventControl::Path(path))
                // TODO: implement hierarchical_event_identifier
                unimplemented!("hierarchical_event_identifier")
            },
            event_control.event_expression(), event_expression => {
                let event_expr = self.lower_event_expr(&event_expression)?;
                Some(EventControl::EventExpr(event_expr))
            },
            event_control.token_star(), _ => Some(EventControl::Star),
            _ => None,
        }
    }
}
