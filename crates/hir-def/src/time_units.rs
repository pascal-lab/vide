use la_arena::Idx;
use syntax::{SyntaxToken, TimeUnit, TokenKind, ast, ast::AstNode};

use crate::{
    alloc_with_source_entry,
    lower::{LoweringCtx, LoweringStore},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeScaleMagnitude {
    One,
    Ten,
    Hundred,
}

impl TimeScaleMagnitude {
    pub(crate) fn from_literal(value: f64) -> Option<Self> {
        match value {
            1.0 => Some(Self::One),
            10.0 => Some(Self::Ten),
            100.0 => Some(Self::Hundred),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeScaleValue {
    pub unit: TimeUnit,
    pub magnitude: TimeScaleMagnitude,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeUnitsKind {
    Unit,
    Precision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeUnitsDecl {
    pub kind: TimeUnitsKind,
    pub value: TimeScaleValue,
    pub precision: Option<TimeScaleValue>,
}

pub type TimeUnitsDeclId = Idx<TimeUnitsDecl>;

pub(crate) fn lower_time_scale_value(token: SyntaxToken<'_>) -> Option<TimeScaleValue> {
    if token.kind() != TokenKind::TIME_LITERAL {
        return None;
    }

    Some(TimeScaleValue {
        unit: token.time_unit()?,
        magnitude: TimeScaleMagnitude::from_literal(token.real()?)?,
    })
}

impl<Store: LoweringStore> LoweringCtx<Store> {
    pub(crate) fn lower_time_units_decl(
        &mut self,
        declaration: ast::TimeUnitsDeclaration<'_>,
    ) -> Option<TimeUnitsDeclId> {
        let kind = match declaration.keyword() {
            Some(keyword) => match keyword.kind() {
                TokenKind::TIME_UNIT_KEYWORD => TimeUnitsKind::Unit,
                TokenKind::TIME_PRECISION_KEYWORD => TimeUnitsKind::Precision,
                _ => {
                    self.report_invalid(
                        declaration.syntax(),
                        "time units declaration has an invalid keyword",
                    );
                    return None;
                }
            },
            None => {
                self.report_invalid(
                    declaration.syntax(),
                    "time units declaration is missing its keyword",
                );
                return None;
            }
        };

        let value = match declaration.time().and_then(lower_time_scale_value) {
            Some(value) => value,
            None => {
                self.report_invalid(
                    declaration.syntax(),
                    "time units declaration has an invalid time scale value",
                );
                return None;
            }
        };

        let precision = match (kind, declaration.divider()) {
            (TimeUnitsKind::Unit, Some(divider)) => {
                match divider.value().and_then(lower_time_scale_value) {
                    Some(value) => Some(value),
                    None => {
                        self.report_invalid(
                            declaration.syntax(),
                            "time units declaration has an invalid precision value",
                        );
                        return None;
                    }
                }
            }
            (TimeUnitsKind::Precision, Some(_)) => {
                self.report_invalid(
                    declaration.syntax(),
                    "timeprecision declaration cannot have a divider",
                );
                return None;
            }
            (TimeUnitsKind::Precision, None) | (TimeUnitsKind::Unit, None) => None,
        };

        let source = self.source_id(declaration.syntax());
        let (body, sources) = self.store.body();
        Some(alloc_with_source_entry(
            &mut body.time_units,
            &mut sources.time_units_srcs,
            TimeUnitsDecl { kind, value, precision },
            source,
        ))
    }
}
