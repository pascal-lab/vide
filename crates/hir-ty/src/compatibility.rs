use hir_def::{
    container::ArenaOwnerId,
    expr::{
        BinaryOp, Expr, ExprId, UnaryOp,
        data_ty::{BuiltinDataTy, Dimension, IntKind},
    },
    literal::Literal,
};

use crate::{
    db::TyDb,
    ty::{BuiltinTy, Ty, TyClass},
    type_system::Compatibility,
};

pub(crate) fn type_class(db: &dyn TyDb, ty: &Ty) -> Option<TyClass> {
    match ty {
        Ty::Alias { target, .. } => type_class(db, target),
        Ty::Builtin(BuiltinTy::Data { id, .. }) => match id.get() {
            BuiltinDataTy::Int { .. } | BuiltinDataTy::Vector { .. } => Some(TyClass::Integral),
            BuiltinDataTy::Real(_) => Some(TyClass::Real),
            BuiltinDataTy::String => Some(TyClass::String),
            BuiltinDataTy::Event | BuiltinDataTy::Chandle | BuiltinDataTy::Void => None,
        },
        Ty::Enum(_) => Some(TyClass::Integral),
        Ty::Unknown
        | Ty::Error
        | Ty::Void
        | Ty::Struct(_)
        | Ty::Union(_)
        | Ty::Queue { .. }
        | Ty::Assoc { .. }
        | Ty::Dynamic(_)
        | Ty::Event
        | Ty::Chandle
        | Ty::Module(_)
        | Ty::Checker(_)
        | Ty::Covergroup(_)
        | Ty::VirtualInterface { .. }
        | Ty::GenerateBlock(_)
        | Ty::Block(_) => None,
    }
}

pub(crate) fn compatibility(db: &dyn TyDb, expected: &Ty, candidate: &Ty) -> Compatibility {
    let (Some(expected_class), Some(candidate_class)) =
        (type_class(db, expected), type_class(db, candidate))
    else {
        return Compatibility::Unknown;
    };
    if expected_class != candidate_class {
        return Compatibility::Incompatible;
    }
    if expected_class != TyClass::Integral {
        return Compatibility::Compatible;
    }

    match (packed_bit_width(db, expected), packed_bit_width(db, candidate)) {
        (Some(expected), Some(candidate)) if expected == candidate => Compatibility::Compatible,
        (Some(_), Some(_)) => Compatibility::Incompatible,
        _ => Compatibility::Unknown,
    }
}

pub(crate) fn is_typed_value(db: &dyn TyDb, ty: &Ty) -> bool {
    type_class(db, ty).is_some()
}

pub(crate) fn packed_bit_width(db: &dyn TyDb, ty: &Ty) -> Option<u64> {
    match ty {
        Ty::Alias { target, .. } => packed_bit_width(db, target),
        Ty::Builtin(BuiltinTy::Data { id, container }) => match id.get() {
            BuiltinDataTy::String
            | BuiltinDataTy::Real(_)
            | BuiltinDataTy::Event
            | BuiltinDataTy::Chandle
            | BuiltinDataTy::Void => None,
            BuiltinDataTy::Int { kind, .. } => Some(int_kind_width(*kind) as u64),
            BuiltinDataTy::Vector { dimensions, .. } => {
                if dimensions.is_empty() {
                    return Some(1);
                }

                let mut product: u64 = 1;
                for dim in dimensions {
                    let dim = (*dim)?;
                    let width = match dim {
                        Dimension::Range(left, right) => {
                            let left = eval_const_i128(db, container, left)?;
                            let right = eval_const_i128(db, container, right)?;
                            i128::abs(left - right).checked_add(1)?
                        }
                        Dimension::Size(size) => eval_const_i128(db, container, size)?,
                        Dimension::Queue(_) | Dimension::Assoc(_) | Dimension::Dynamic => {
                            return None;
                        }
                    };
                    let width: u64 = width.try_into().ok()?;
                    product = product.checked_mul(width)?;
                }
                Some(product)
            }
        },
        Ty::Unknown
        | Ty::Error
        | Ty::Void
        | Ty::Struct(_)
        | Ty::Enum(_)
        | Ty::Union(_)
        | Ty::Queue { .. }
        | Ty::Assoc { .. }
        | Ty::Dynamic(_)
        | Ty::Event
        | Ty::Chandle
        | Ty::Module(_)
        | Ty::Checker(_)
        | Ty::Covergroup(_)
        | Ty::VirtualInterface { .. }
        | Ty::GenerateBlock(_)
        | Ty::Block(_) => None,
    }
}

fn int_kind_width(kind: IntKind) -> usize {
    match kind {
        IntKind::Byte => 8,
        IntKind::ShortInt => 16,
        IntKind::Int => 32,
        IntKind::LongInt => 64,
        IntKind::Integer => 32,
        IntKind::Time => 64,
    }
}

fn eval_const_i128(db: &dyn TyDb, container: &ArenaOwnerId, expr_id: ExprId) -> Option<i128> {
    let data = container.data(db);
    match data.expr(expr_id) {
        Expr::Literal(Literal::Int(int)) => int.get_single_word().map(|value| value as i128),
        Expr::Unary { op, expr } => {
            let value = eval_const_i128(db, container, *expr)?;
            match op {
                UnaryOp::Pos => Some(value),
                UnaryOp::Neg => value.checked_neg(),
                _ => None,
            }
        }
        Expr::Binary { op, lhs, rhs } => {
            let left = eval_const_i128(db, container, *lhs)?;
            let right = eval_const_i128(db, container, *rhs)?;
            match op {
                BinaryOp::Add => left.checked_add(right),
                BinaryOp::Sub => left.checked_sub(right),
                BinaryOp::Mul => left.checked_mul(right),
                BinaryOp::Div => (right != 0).then(|| left.checked_div(right)).flatten(),
                BinaryOp::Mod => (right != 0).then(|| left.checked_rem(right)).flatten(),
                BinaryOp::ShiftLeft => {
                    u32::try_from(right).ok().and_then(|shift| left.checked_shl(shift))
                }
                BinaryOp::ShiftRight => {
                    u32::try_from(right).ok().and_then(|shift| left.checked_shr(shift))
                }
                _ => None,
            }
        }
        Expr::Cast { expr, .. } | Expr::SignedCast { expr, .. } => {
            eval_const_i128(db, container, *expr)
        }
        _ => None,
    }
}
