use hir::{
    db::{HirDb, InternDb},
    hir_def::{
        Ident,
        declaration::Declaration,
        expr::{
            BinaryOp, Expr, ExprId, UnaryOp,
            data_ty::{BuiltinDataTy, DataTy, Dimension, IntKind},
        },
        literal::Literal,
        module::ModuleId,
    },
    scope::ModuleEntry,
};
use ide_db::root_db::RootDb;
use utils::get::{Get, GetRef};

pub(super) fn expected_port_ty(
    db: &RootDb,
    target_module: &hir::hir_def::module::Module,
    target_module_id: ModuleId,
    port_name: &Ident,
) -> Option<DataTy> {
    let scope = db.module_scope(target_module_id);
    let entry = scope.get(port_name)?;

    match entry {
        ModuleEntry::AnsiPortEntry(hir::scope::AnsiPortEntry(decl_id)) => {
            decl_ty_in_module(target_module, decl_id)
        }
        ModuleEntry::NonAnsiPortEntry(entry) => {
            let decl_id = entry.data_decl.or(entry.port_decl)?;
            decl_ty_in_module(target_module, decl_id)
        }
        _ => None,
    }
}

pub(super) fn expected_param_ty(
    db: &RootDb,
    target_module: &hir::hir_def::module::Module,
    target_module_id: ModuleId,
    param_name: &Ident,
) -> Option<DataTy> {
    let scope = db.module_scope(target_module_id);
    let ModuleEntry::DeclId(decl_id) = scope.get(param_name)? else {
        return None;
    };

    let hir::hir_def::expr::declarator::DeclaratorParent::DeclarationId(declaration_id) =
        target_module.get(decl_id).parent
    else {
        return None;
    };
    let Declaration::ParamDecl(param_decl) = target_module.get(declaration_id) else {
        return None;
    };

    is_overridable_parameter_decl(db, target_module_id, declaration_id).then_some(param_decl.ty)
}

pub(super) fn value_candidates_in_module(
    db: &RootDb,
    module_id: ModuleId,
) -> Vec<(String, DataTy)> {
    let module = db.module(module_id);
    let mut candidates: Vec<(String, DataTy)> = Vec::new();

    for (_, decl) in module.declarations.iter() {
        let ty = decl.ty();
        match decl {
            Declaration::DataDecl(_) | Declaration::NetDecl(_) => {
                for decl_id in decl.decls().clone() {
                    if let Some(name) = module.get(decl_id).name.as_ref() {
                        candidates.push((name.to_string(), ty));
                    }
                }
            }
            Declaration::ParamDecl(_) => {}
        }
    }

    match &module.ports {
        hir::hir_def::module::port::Ports::Ansi(port_decls) => {
            for (_, port_decl) in port_decls.iter() {
                let ty = port_decl.header.ty();
                for decl_id in port_decl.decls.clone() {
                    if let Some(name) = module.get(decl_id).name.as_ref() {
                        candidates.push((name.to_string(), ty));
                    }
                }
            }
        }
        hir::hir_def::module::port::Ports::NonAnsi { decls, .. } => {
            for (_, port_decl) in decls.iter() {
                let ty = port_decl.header.ty();
                for decl_id in port_decl.decls.clone() {
                    if let Some(name) = module.get(decl_id).name.as_ref() {
                        candidates.push((name.to_string(), ty));
                    }
                }
            }
        }
    }

    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    candidates.dedup_by(|a, b| a.0 == b.0);
    candidates
}

pub(super) fn const_candidates_in_module(
    db: &RootDb,
    module_id: ModuleId,
) -> Vec<(String, DataTy)> {
    let module = db.module(module_id);
    let mut candidates: Vec<(String, DataTy)> = Vec::new();

    for (_, decl) in module.declarations.iter() {
        let Declaration::ParamDecl(param_decl) = decl else {
            continue;
        };
        for decl_id in param_decl.decls.clone() {
            if let Some(name) = module.get(decl_id).name.as_ref() {
                candidates.push((name.to_string(), param_decl.ty));
            }
        }
    }

    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    candidates.dedup_by(|a, b| a.0 == b.0);
    candidates
}

pub(super) fn is_compatible_typed_value(
    db: &RootDb,
    expected_module: &hir::hir_def::module::Module,
    expected_ty: DataTy,
    candidate_module: &hir::hir_def::module::Module,
    candidate_ty: DataTy,
) -> bool {
    let (Some(expected_class), Some(candidate_class)) =
        (type_class(db, expected_ty), type_class(db, candidate_ty))
    else {
        return false;
    };
    if expected_class != candidate_class {
        return false;
    }

    if expected_class != TyClass::Integral {
        return true;
    }

    let expected_w = packed_bit_width(db, expected_module, expected_ty);
    let candidate_w = packed_bit_width(db, candidate_module, candidate_ty);
    match (expected_w, candidate_w) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

fn decl_ty_in_module(
    module: &hir::hir_def::module::Module,
    decl_id: hir::hir_def::expr::declarator::DeclId,
) -> Option<DataTy> {
    use hir::hir_def::expr::declarator::DeclaratorParent;
    match module.get(decl_id).parent {
        DeclaratorParent::PortDeclId(port_decl_id) => {
            Some(module.ports.get(port_decl_id).header.ty())
        }
        DeclaratorParent::DeclarationId(declaration_id) => Some(module.get(declaration_id).ty()),
        DeclaratorParent::StmtId(_) => None,
    }
}

fn is_overridable_parameter_decl(
    db: &RootDb,
    module_id: ModuleId,
    declaration_id: hir::hir_def::declaration::DeclarationId,
) -> bool {
    let (_, module_src_map) = db.module_with_source_map(module_id);
    let tree = db.parse(module_id.file_id);
    let src = module_src_map.get(declaration_id);
    let hir::hir_def::declaration::DeclarationSrc::ParameterDeclaration(ptr) = src else {
        return false;
    };
    let Some(node) = ptr.to_node(&tree) else {
        return false;
    };
    node.first_token().is_some_and(|kw| kw.kind() == syntax::Token![parameter])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TyClass {
    Integral,
    Real,
    String,
}

fn type_class(db: &RootDb, ty: DataTy) -> Option<TyClass> {
    let DataTy::Builtin(id) = ty else {
        return None;
    };
    match db.lookup_intern_ty(id) {
        BuiltinDataTy::Int { .. } | BuiltinDataTy::Vector { .. } => Some(TyClass::Integral),
        BuiltinDataTy::Real(_) => Some(TyClass::Real),
        BuiltinDataTy::String => Some(TyClass::String),
        BuiltinDataTy::Void => None,
    }
}

fn packed_bit_width(db: &RootDb, module: &hir::hir_def::module::Module, ty: DataTy) -> Option<u64> {
    let DataTy::Builtin(id) = ty else {
        return None;
    };
    let builtin = db.lookup_intern_ty(id);
    match builtin {
        BuiltinDataTy::String | BuiltinDataTy::Real(_) | BuiltinDataTy::Void => None,
        BuiltinDataTy::Int { kind, .. } => Some(int_kind_width(kind) as u64),
        BuiltinDataTy::Vector { dimensions, .. } => {
            if dimensions.is_empty() {
                return Some(1);
            }

            let mut product: u64 = 1;
            for dim in dimensions {
                let dim = dim?;
                let width = match dim {
                    Dimension::Range(left, right) => {
                        let l = eval_const_i128(module, left)?;
                        let r = eval_const_i128(module, right)?;
                        i128::abs(l - r).checked_add(1)?
                    }
                    Dimension::Size(size) => eval_const_i128(module, size)?,
                };
                let width: u64 = width.try_into().ok()?;
                product = product.checked_mul(width)?;
            }
            Some(product)
        }
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

fn eval_const_i128(module: &hir::hir_def::module::Module, expr_id: ExprId) -> Option<i128> {
    match module.get(expr_id) {
        Expr::Literal(Literal::Int(int)) => int.get_single_word().map(|v| v as i128),
        Expr::Unary { op, expr } => {
            let v = eval_const_i128(module, *expr)?;
            match op {
                UnaryOp::Pos => Some(v),
                UnaryOp::Neg => Some(v.checked_neg()?),
                _ => None,
            }
        }
        Expr::Binary { op, lhs, rhs } => {
            let l = eval_const_i128(module, *lhs)?;
            let r = eval_const_i128(module, *rhs)?;
            match op {
                BinaryOp::Add => l.checked_add(r),
                BinaryOp::Sub => l.checked_sub(r),
                BinaryOp::Mul => l.checked_mul(r),
                BinaryOp::Div => (r != 0).then(|| l.checked_div(r)).flatten(),
                BinaryOp::Mod => (r != 0).then(|| l.checked_rem(r)).flatten(),
                BinaryOp::ShiftLeft => u32::try_from(r).ok().and_then(|s| l.checked_shl(s)),
                BinaryOp::ShiftRight => u32::try_from(r).ok().and_then(|s| l.checked_shr(s)),
                _ => None,
            }
        }
        Expr::Cast { expr, .. } | Expr::SignedCast { expr, .. } => eval_const_i128(module, *expr),
        _ => None,
    }
}
