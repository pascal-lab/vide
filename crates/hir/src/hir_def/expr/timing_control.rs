use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{TokenKind, ast};

use super::ExprId;
use crate::{
    hir_def::{
        alloc_with_source,
        lower::{LoweringCtx, LoweringStore},
    },
    source_map::{AstId, AstKind},
};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum TimingControl {
    DelayControl(DelayControl),
    EventControl(EventControl),
    RepeatedEventControl(ExprId, Option<EventControl>),
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum DelayControl {
    OneStep,
    Value(ExprId),
    Delay3(SmallVec<[ExprId; 3]>),
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum EventControl {
    // @(*)
    Implicit,
    Event(ExprId),
    EventExpr(EventExprId),
}

// EventExpressions

pub type EventExprId = Idx<EventExpr>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct EventExpressionAst;

impl AstKind for EventExpressionAst {
    type Node<'a> = ast::EventExpression<'a>;
}

pub type EventExprSrc = AstId<EventExpressionAst>;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum EventExpr {
    Missing,
    Atom { sensitivity: Option<Sensitivity>, expr: ExprId, iff: Option<ExprId> },
    Or(EventExprId, EventExprId),
    And(EventExprId, EventExprId),
    // TODO: sequence_instance
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Sensitivity {
    Posedge,
    Negedge,
    Edge,
}

impl<Store: LoweringStore> LoweringCtx<'_, Store> {
    pub(crate) fn lower_event_expr(&mut self, event_expr: ast::EventExpression) -> EventExprId {
        let hir_event_expr = self.lower_event_expr_inner(event_expr);
        let file_id = self.file_id;
        let (event_expressions, sources) = self.event_expressions();
        alloc_with_source(file_id, event_expressions, sources, hir_event_expr, event_expr)
    }

    fn lower_event_expr_inner(&mut self, event_expr: ast::EventExpression) -> EventExpr {
        use ast::EventExpression::*;
        match event_expr {
            ParenthesizedEventExpression(event_expr) => {
                self.lower_event_expr_inner(event_expr.expr())
            }
            BinaryEventExpression(event_expr) => self.lower_binary_event_expr(event_expr),
            SignalEventExpression(event_expr) => self.lower_signal_event_expr(event_expr),
        }
    }

    fn lower_binary_event_expr(&mut self, event_expr: ast::BinaryEventExpression) -> EventExpr {
        let left = self.lower_event_expr(event_expr.left());
        let right = self.lower_event_expr(event_expr.right());
        match event_expr.operator_token().map(|tok| tok.kind()) {
            Some(TokenKind::OR_KEYWORD) => EventExpr::Or(left, right),
            Some(TokenKind::COMMA) => EventExpr::And(left, right),
            _ => EventExpr::Missing,
        }
    }

    fn lower_signal_event_expr(&mut self, event_expr: ast::SignalEventExpression) -> EventExpr {
        let sensitivity = event_expr.edge().and_then(|tok| match tok.kind() {
            TokenKind::POS_EDGE_KEYWORD => Some(Sensitivity::Posedge),
            TokenKind::NEG_EDGE_KEYWORD => Some(Sensitivity::Negedge),
            TokenKind::EDGE_KEYWORD => Some(Sensitivity::Edge),
            _ => None,
        });
        let expr = self.lower_expr(event_expr.expr());
        let iff = event_expr.iff_clause().map(|iff| self.lower_expr(iff.expr()));
        EventExpr::Atom { sensitivity, expr, iff }
    }
}

impl<Store: LoweringStore> LoweringCtx<'_, Store> {
    pub(crate) fn lower_timing_control(&mut self, control: ast::TimingControl) -> TimingControl {
        match control {
            ast::TimingControl::OneStepDelay(_) => {
                TimingControl::DelayControl(DelayControl::OneStep)
            }
            ast::TimingControl::Delay(delay) => {
                TimingControl::DelayControl(self.lower_delay(delay))
            }
            ast::TimingControl::RepeatedEventControl(event_control) => {
                self.lower_repeated_event_control(event_control)
            }
            ast::TimingControl::EventControl(event_control) => {
                TimingControl::EventControl(self.lower_event_control(event_control))
            }
            ast::TimingControl::Delay3(delay3) => {
                TimingControl::DelayControl(self.lower_delay3(delay3))
            }
            ast::TimingControl::ImplicitEventControl(_) => {
                TimingControl::EventControl(EventControl::Implicit)
            }
            ast::TimingControl::EventControlWithExpression(expr) => {
                TimingControl::EventControl(self.lower_event_control_with_expr(expr))
            }
        }
    }

    fn lower_delay(&mut self, delay: ast::Delay) -> DelayControl {
        let val = self.lower_expr(delay.delay_value());

        match delay {
            ast::Delay::CycleDelay(_) | ast::Delay::DelayControl(_) => DelayControl::Value(val),
        }
    }

    fn lower_delay3(&mut self, delay: ast::Delay3) -> DelayControl {
        let mut delays = SmallVec::new();
        delays.push(self.lower_expr(delay.delay_1()));

        if let Some(delay_2) = delay.delay_2() {
            delays.push(self.lower_expr(delay_2));
        }

        if let Some(delay_3) = delay.delay_3() {
            delays.push(self.lower_expr(delay_3));
        }

        DelayControl::Delay3(delays)
    }

    fn lower_event_control_with_expr(
        &mut self,
        event_control: ast::EventControlWithExpression,
    ) -> EventControl {
        EventControl::EventExpr(self.lower_event_expr(event_control.expr()))
    }

    fn lower_event_control(&mut self, event_control: ast::EventControl) -> EventControl {
        EventControl::Event(self.lower_expr(event_control.event_name()))
    }

    fn lower_repeated_event_control(
        &mut self,
        event_control: ast::RepeatedEventControl,
    ) -> TimingControl {
        let expr = self.lower_expr(event_control.expr());
        let event_control = event_control.event_control().and_then(|control| {
            match self.lower_timing_control(control) {
                TimingControl::EventControl(event_control) => Some(event_control),
                _ => None,
            }
        });
        TimingControl::RepeatedEventControl(expr, event_control)
    }
}
