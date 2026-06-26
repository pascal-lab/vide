use rustc_hash::FxHashSet;
use triomphe::Arc;
use utils::get::GetRef;

use crate::{
    container::{InContainer, InSubroutine, ScopeId},
    db::HirDb,
    hir_def::{
        Ident,
        aggregate::StructId,
        declaration::Declaration,
        expr::{
            BinaryOp, Expr, ExprId, UnaryOp,
            data_ty::{BuiltinDataTy, BuiltinDataTyId, DataTy, Dimension, IntKind, NamedDataTy},
            declarator::{DeclId, DeclaratorParent},
        },
        literal::Literal,
        module::{ModuleId, ModuleKind, generate::GenerateBlockId, port::PortDeclId},
        stmt::{ForInit, StmtKind},
        subroutine::SubroutinePortId,
        typedef::TypedefId,
    },
    semantics::pathres::{PathResolution, resolve_name},
    symbol::{DefId, DefKind, NameContext},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinTy {
    Data { id: BuiltinDataTyId, container: ScopeId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    Unknown,
    Error,
    Void,
    Builtin(BuiltinTy),
    Struct(InContainer<StructId>),
    Alias { typedef: InContainer<TypedefId>, target: Box<Ty> },
    Module(ModuleId),
    GenerateBlock(GenerateBlockId),
    Block(crate::hir_def::block::BlockId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TyResult {
    pub ty: Ty,
    pub diagnostics: Vec<TyInferDiagnostic>,
}

impl TyResult {
    fn new(ty: Ty) -> Self {
        TyResult { ty, diagnostics: Vec::new() }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TyInferDiagnostic {
    TypedefCycle(InContainer<TypedefId>),
}

#[derive(Debug, Clone)]
pub struct TyMember {
    pub name: Ident,
    pub ty: Ty,
    pub origin: Option<PathResolution>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TyClass {
    Integral,
    Real,
    String,
}

pub fn normalize_data_ty(db: &dyn HirDb, container: ScopeId, data_ty: DataTy) -> TyResult {
    normalize_data_ty_inner(db, container, data_ty, &mut FxHashSet::default())
}

pub(crate) fn type_of_typedef_query(
    db: &dyn HirDb,
    typedef: InContainer<TypedefId>,
) -> Arc<TyResult> {
    Arc::new(type_of_typedef_impl(db, typedef))
}

pub(crate) fn type_of_decl_query(db: &dyn HirDb, decl: InContainer<DeclId>) -> Arc<TyResult> {
    Arc::new(type_of_decl_impl(db, decl))
}

pub(crate) fn type_of_path_resolution_query(db: &dyn HirDb, res: PathResolution) -> Arc<TyResult> {
    Arc::new(type_of_path_resolution_impl(db, res))
}

pub(crate) fn type_of_expr_query(db: &dyn HirDb, expr: InContainer<ExprId>) -> Arc<TyResult> {
    Arc::new(type_of_expr_impl(db, expr))
}

pub(crate) fn type_of_subroutine_port_query(
    db: &dyn HirDb,
    port: InSubroutine<SubroutinePortId>,
) -> Arc<TyResult> {
    Arc::new(type_of_subroutine_port_impl(db, port))
}

fn type_of_typedef_impl(db: &dyn HirDb, typedef: InContainer<TypedefId>) -> TyResult {
    type_of_typedef_inner(db, typedef, &mut FxHashSet::default())
}

fn type_of_decl_impl(db: &dyn HirDb, decl: InContainer<DeclId>) -> TyResult {
    let Some(data_ty) = data_ty_of_decl(db, decl) else {
        return TyResult::new(Ty::Unknown);
    };
    normalize_data_ty(db, decl.cont_id, data_ty)
}

fn type_of_path_resolution_impl(db: &dyn HirDb, res: PathResolution) -> TyResult {
    let mut port_ty = None;
    for def_id in res.def_ids() {
        let ty = type_of_def_id(db, *def_id);
        match def_id.kind(db) {
            DefKind::NonAnsiPort => {}
            DefKind::Port | DefKind::SubroutinePort => {
                port_ty.get_or_insert(ty);
            }
            _ if !matches!(ty.ty, Ty::Unknown) => return ty,
            _ => {}
        }
    }
    port_ty.unwrap_or_else(|| TyResult::new(Ty::Unknown))
}

fn type_of_def_id(db: &dyn HirDb, def_id: DefId) -> TyResult {
    match def_id.kind(db) {
        DefKind::Module | DefKind::Package => def_id
            .as_module(db)
            .map(|module_id| TyResult::new(Ty::Module(module_id)))
            .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
        DefKind::Port
        | DefKind::Variable
        | DefKind::Net
        | DefKind::Param
        | DefKind::Genvar
        | DefKind::Specparam => def_id
            .as_decl(db)
            .map(|decl| type_of_decl_impl(db, decl))
            .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
        DefKind::Typedef | DefKind::Enum | DefKind::Struct => def_id
            .as_typedef(db)
            .map(|typedef| type_of_typedef_impl(db, typedef))
            .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
        DefKind::SubroutinePort => def_id
            .as_subroutine_port(db)
            .map(|port| type_of_subroutine_port_impl(db, port))
            .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
        DefKind::Instance => def_id
            .as_instance(db)
            .and_then(|instance| instance_target_module_id(db, instance.module_id, instance.value))
            .map(|module_id| TyResult::new(Ty::Module(module_id)))
            .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
        DefKind::GenerateBlock => def_id
            .as_generate_block(db)
            .map(|generate_block_id| TyResult::new(Ty::GenerateBlock(generate_block_id)))
            .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
        DefKind::Block => def_id
            .as_block(db)
            .map(|block_id| TyResult::new(Ty::Block(block_id)))
            .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
        DefKind::Interface
        | DefKind::Program
        | DefKind::Class
        | DefKind::Covergroup
        | DefKind::Checker
        | DefKind::Udp
        | DefKind::Config
        | DefKind::Library
        | DefKind::Subroutine
        | DefKind::NonAnsiPort
        | DefKind::ClassField
        | DefKind::Method
        | DefKind::Modport
        | DefKind::ClockingBlock
        | DefKind::Sequence
        | DefKind::Property
        | DefKind::Stmt => TyResult::new(Ty::Unknown),
    }
}

fn type_of_expr_impl(db: &dyn HirDb, expr: InContainer<ExprId>) -> TyResult {
    let Some(hir_expr) = expr_of(db, expr) else {
        return TyResult::new(Ty::Unknown);
    };

    match hir_expr {
        Expr::Ident(ident) => resolve_name(db, expr.cont_id, &ident, NameContext::Value)
            .map(|res| type_of_path_resolution_impl(db, res))
            .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
        Expr::Field { receiver, field } => {
            let Some(field) = field else {
                return TyResult::new(Ty::Unknown);
            };
            let base = type_of_expr_impl(db, expr.with_value(receiver));
            if matches!(base.ty, Ty::Unknown | Ty::Error) {
                return base;
            }
            let mut selected = select_member(db, &base.ty, &field);
            selected.diagnostics.extend(base.diagnostics);
            selected
        }
        Expr::ElementSelect { receiver, .. } => type_of_expr_impl(db, expr.with_value(receiver)),
        Expr::Cast { ty, .. } => normalize_data_ty(db, expr.cont_id, ty),
        _ => TyResult::new(Ty::Unknown),
    }
}

pub fn members_of_ty(db: &dyn HirDb, ty: &Ty) -> Vec<TyMember> {
    match ty {
        Ty::Alias { target, .. } => members_of_ty(db, target),
        Ty::Struct(struct_id) => struct_members(db, *struct_id),
        Ty::Module(module_id) => module_members(db, *module_id),
        Ty::GenerateBlock(generate_block_id) => generate_block_members(db, *generate_block_id),
        Ty::Block(block_id) => block_members(db, *block_id),
        Ty::Unknown | Ty::Error | Ty::Void | Ty::Builtin(_) => Vec::new(),
    }
}

pub fn select_member(db: &dyn HirDb, base: &Ty, name: &Ident) -> TyResult {
    members_of_ty(db, base)
        .into_iter()
        .find(|member| &member.name == name)
        .map(|member| TyResult::new(member.ty))
        .unwrap_or_else(|| TyResult::new(Ty::Unknown))
}

pub fn type_class(db: &dyn HirDb, ty: &Ty) -> Option<TyClass> {
    match ty {
        Ty::Alias { target, .. } => type_class(db, target),
        Ty::Builtin(BuiltinTy::Data { id, .. }) => match db.lookup_intern_ty(*id) {
            BuiltinDataTy::Int { .. } | BuiltinDataTy::Vector { .. } => Some(TyClass::Integral),
            BuiltinDataTy::Real(_) => Some(TyClass::Real),
            BuiltinDataTy::String => Some(TyClass::String),
            BuiltinDataTy::Void => None,
        },
        Ty::Unknown
        | Ty::Error
        | Ty::Void
        | Ty::Struct(_)
        | Ty::Module(_)
        | Ty::GenerateBlock(_)
        | Ty::Block(_) => None,
    }
}

pub fn is_compatible_ty(db: &dyn HirDb, expected: &Ty, candidate: &Ty) -> bool {
    let (Some(expected_class), Some(candidate_class)) =
        (type_class(db, expected), type_class(db, candidate))
    else {
        return true;
    };
    if expected_class != candidate_class {
        return false;
    }

    if expected_class != TyClass::Integral {
        return true;
    }

    match (packed_bit_width(db, expected), packed_bit_width(db, candidate)) {
        (Some(expected), Some(candidate)) => expected == candidate,
        _ => true,
    }
}

pub fn packed_bit_width(db: &dyn HirDb, ty: &Ty) -> Option<u64> {
    match ty {
        Ty::Alias { target, .. } => packed_bit_width(db, target),
        Ty::Builtin(BuiltinTy::Data { id, container }) => match db.lookup_intern_ty(*id) {
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
                            let left = eval_const_i128(db, *container, left)?;
                            let right = eval_const_i128(db, *container, right)?;
                            i128::abs(left - right).checked_add(1)?
                        }
                        Dimension::Size(size) => eval_const_i128(db, *container, size)?,
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
        | Ty::Module(_)
        | Ty::GenerateBlock(_)
        | Ty::Block(_) => None,
    }
}

fn normalize_data_ty_inner(
    db: &dyn HirDb,
    container: ScopeId,
    data_ty: DataTy,
    seen: &mut FxHashSet<InContainer<TypedefId>>,
) -> TyResult {
    match data_ty {
        DataTy::Builtin(builtin) => {
            if matches!(
                db.lookup_intern_ty(builtin),
                crate::hir_def::expr::data_ty::BuiltinDataTy::Void
            ) {
                TyResult::new(Ty::Void)
            } else {
                TyResult::new(Ty::Builtin(BuiltinTy::Data { id: builtin, container }))
            }
        }
        DataTy::Struct(struct_id) => TyResult::new(Ty::Struct(struct_id)),
        DataTy::Named(named) => type_of_named_data_ty(db, container, named, seen),
    }
}

fn type_of_named_data_ty(
    db: &dyn HirDb,
    container: ScopeId,
    named: NamedDataTy,
    seen: &mut FxHashSet<InContainer<TypedefId>>,
) -> TyResult {
    let expr_id = match named {
        NamedDataTy::Ident(expr_id) | NamedDataTy::Field(expr_id) => expr_id,
    };
    let Some(Expr::Ident(ident)) = expr_of(db, InContainer::new(container, expr_id)) else {
        return TyResult::new(Ty::Unknown);
    };

    match resolve_name(db, container, &ident, NameContext::Type) {
        Some(res) => {
            for def_id in res.def_ids() {
                if let Some(typedef) = def_id.as_typedef(db) {
                    return type_of_typedef_inner(db, typedef, seen);
                }
            }
            type_of_path_resolution_impl(db, res)
        }
        None => TyResult::new(Ty::Unknown),
    }
}

fn type_of_typedef_inner(
    db: &dyn HirDb,
    typedef: InContainer<TypedefId>,
    seen: &mut FxHashSet<InContainer<TypedefId>>,
) -> TyResult {
    if !seen.insert(typedef) {
        return TyResult {
            ty: Ty::Error,
            diagnostics: vec![TyInferDiagnostic::TypedefCycle(typedef)],
        };
    }

    let Some(def) = typedef_of(db, typedef) else {
        seen.remove(&typedef);
        return TyResult::new(Ty::Unknown);
    };
    let Some(data_ty) = def.ty else {
        seen.remove(&typedef);
        return TyResult::new(Ty::Unknown);
    };

    let mut target = normalize_data_ty_inner(db, typedef.cont_id, data_ty, seen);
    seen.remove(&typedef);
    let ty = if matches!(target.ty, Ty::Error) {
        Ty::Error
    } else {
        Ty::Alias { typedef, target: Box::new(target.ty) }
    };
    TyResult { ty, diagnostics: std::mem::take(&mut target.diagnostics) }
}

fn struct_members(db: &dyn HirDb, struct_id: InContainer<StructId>) -> Vec<TyMember> {
    let Some(def) = struct_of(db, struct_id) else {
        return Vec::new();
    };

    def.members
        .iter()
        .filter_map(|member| {
            let name = member.name.clone()?;
            let ty = member
                .ty
                .map(|ty| normalize_data_ty(db, ty.cont_id, ty.value).ty)
                .unwrap_or(Ty::Unknown);
            Some(TyMember { name, ty, origin: None })
        })
        .collect()
}

fn module_members(db: &dyn HirDb, module_id: ModuleId) -> Vec<TyMember> {
    let file = db.hir_file(crate::file::HirFileId::File(module_id.file_id()));
    let scope = if file.get(module_id.value).kind == ModuleKind::Package {
        db.package_export_scope(module_id)
    } else {
        db.module_scope(module_id)
    };

    let mut members: Vec<_> = scope
        .iter_listing()
        .filter_map(|(name, defs)| {
            let origin = PathResolution::from_def_ids(defs)?;
            let ty = type_of_path_resolution_impl(db, origin.clone()).ty;
            Some(TyMember { name: name.clone(), ty, origin: Some(origin) })
        })
        .collect();
    sort_members(&mut members);
    members
}

fn generate_block_members(db: &dyn HirDb, generate_block_id: GenerateBlockId) -> Vec<TyMember> {
    let mut members: Vec<_> = db
        .generate_block_scope(generate_block_id)
        .iter_listing()
        .filter_map(|(name, defs)| {
            let origin = PathResolution::from_def_ids(defs)?;
            let ty = type_of_path_resolution_impl(db, origin.clone()).ty;
            Some(TyMember { name: name.clone(), ty, origin: Some(origin) })
        })
        .collect();
    sort_members(&mut members);
    members
}

fn block_members(db: &dyn HirDb, block_id: crate::hir_def::block::BlockId) -> Vec<TyMember> {
    let mut members: Vec<_> = db
        .block_scope(block_id)
        .iter_listing()
        .filter_map(|(name, defs)| {
            let origin = PathResolution::from_def_ids(defs)?;
            let ty = type_of_path_resolution_impl(db, origin.clone()).ty;
            Some(TyMember { name: name.clone(), ty, origin: Some(origin) })
        })
        .collect();
    sort_members(&mut members);
    members
}

fn sort_members(members: &mut Vec<TyMember>) {
    members.sort_by(|left, right| left.name.cmp(&right.name));
    members.dedup_by(|left, right| left.name == right.name);
}

fn data_ty_of_decl(db: &dyn HirDb, decl: InContainer<DeclId>) -> Option<DataTy> {
    let declarator = decl_of(db, decl)?;
    match declarator.parent {
        DeclaratorParent::DeclarationId(declaration_id) => {
            Some(declaration_of(db, decl.with_value(declaration_id))?.ty())
        }
        DeclaratorParent::PortDeclId(port_decl_id) => port_decl_ty(db, decl.cont_id, port_decl_id),
        DeclaratorParent::StmtId(stmt_id) => {
            for_init_decl_ty(db, decl.cont_id, stmt_id, decl.value)
        }
    }
}

fn port_decl_ty(db: &dyn HirDb, cont_id: ScopeId, port_decl_id: PortDeclId) -> Option<DataTy> {
    let ScopeId::Module(module_id) = cont_id else {
        return None;
    };
    let module = db.module(module_id);
    Some(module.ports.get(port_decl_id).header.ty())
}

fn for_init_decl_ty(
    db: &dyn HirDb,
    cont_id: ScopeId,
    stmt_id: crate::hir_def::stmt::StmtId,
    decl_id: DeclId,
) -> Option<DataTy> {
    let stmt = stmt_of(db, InContainer::new(cont_id, stmt_id))?;
    let StmtKind::For { inits: ForInit::Init(inits), .. } = &stmt.kind else {
        return None;
    };
    inits.iter().find_map(|(ty, decl)| (*decl == decl_id).then_some(*ty).flatten())
}

fn type_of_subroutine_port_impl(db: &dyn HirDb, port: InSubroutine<SubroutinePortId>) -> TyResult {
    let subroutine = db.subroutine(port.subroutine);
    let port_id = port;
    let Some(port) = subroutine.ports.get(port_id.value.0 as usize) else {
        return TyResult::new(Ty::Unknown);
    };
    port.ty
        .map(|ty| normalize_data_ty(db, ScopeId::Subroutine(port_id.subroutine), ty))
        .unwrap_or_else(|| TyResult::new(Ty::Unknown))
}

fn instance_target_module_id(
    db: &dyn HirDb,
    module_id: ModuleId,
    instance_id: crate::hir_def::module::instantiation::InstanceId,
) -> Option<ModuleId> {
    let module = db.module(module_id);
    let instance = module.get(instance_id);
    let instantiation = module.get(instance.parent);
    let module_name = instantiation.module_name.as_ref()?;
    db.unit_scope().module_ids(db, module_name).unique()
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

fn eval_const_i128(db: &dyn HirDb, container: ScopeId, expr_id: ExprId) -> Option<i128> {
    match expr_of(db, InContainer::new(container, expr_id))? {
        Expr::Literal(Literal::Int(int)) => int.get_single_word().map(|v| v as i128),
        Expr::Unary { op, expr } => {
            let value = eval_const_i128(db, container, expr)?;
            match op {
                UnaryOp::Pos => Some(value),
                UnaryOp::Neg => Some(value.checked_neg()?),
                _ => None,
            }
        }
        Expr::Binary { op, lhs, rhs } => {
            let left = eval_const_i128(db, container, lhs)?;
            let right = eval_const_i128(db, container, rhs)?;
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
            eval_const_i128(db, container, expr)
        }
        _ => None,
    }
}

fn expr_of(db: &dyn HirDb, expr: InContainer<ExprId>) -> Option<Expr> {
    match expr.cont_id {
        ScopeId::File(file_id) => Some(db.hir_file(file_id).get(expr.value).clone()),
        ScopeId::Module(module_id) => Some(db.module(module_id).get(expr.value).clone()),
        ScopeId::GenerateBlock(generate_block_id) => {
            Some(db.generate_block(generate_block_id).get(expr.value).clone())
        }
        ScopeId::Block(block_id) => Some(db.block(block_id).get(expr.value).clone()),
        ScopeId::Subroutine(subroutine_id) => {
            Some(db.subroutine(subroutine_id).get(expr.value).clone())
        }
    }
}

fn decl_of(
    db: &dyn HirDb,
    decl: InContainer<DeclId>,
) -> Option<crate::hir_def::expr::declarator::Declarator> {
    match decl.cont_id {
        ScopeId::File(file_id) => Some(db.hir_file(file_id).get(decl.value).clone()),
        ScopeId::Module(module_id) => Some(db.module(module_id).get(decl.value).clone()),
        ScopeId::GenerateBlock(generate_block_id) => {
            Some(db.generate_block(generate_block_id).get(decl.value).clone())
        }
        ScopeId::Block(block_id) => Some(db.block(block_id).get(decl.value).clone()),
        ScopeId::Subroutine(subroutine_id) => {
            Some(db.subroutine(subroutine_id).get(decl.value).clone())
        }
    }
}

fn declaration_of(
    db: &dyn HirDb,
    decl: InContainer<crate::hir_def::declaration::DeclarationId>,
) -> Option<Declaration> {
    match decl.cont_id {
        ScopeId::File(file_id) => Some(db.hir_file(file_id).get(decl.value).clone()),
        ScopeId::Module(module_id) => Some(db.module(module_id).get(decl.value).clone()),
        ScopeId::GenerateBlock(generate_block_id) => {
            Some(db.generate_block(generate_block_id).get(decl.value).clone())
        }
        ScopeId::Block(block_id) => Some(db.block(block_id).get(decl.value).clone()),
        ScopeId::Subroutine(subroutine_id) => {
            Some(db.subroutine(subroutine_id).get(decl.value).clone())
        }
    }
}

fn typedef_of(
    db: &dyn HirDb,
    typedef: InContainer<TypedefId>,
) -> Option<crate::hir_def::typedef::Typedef> {
    match typedef.cont_id {
        ScopeId::File(file_id) => Some(db.hir_file(file_id).get(typedef.value).clone()),
        ScopeId::Module(module_id) => Some(db.module(module_id).get(typedef.value).clone()),
        ScopeId::GenerateBlock(generate_block_id) => {
            Some(db.generate_block(generate_block_id).get(typedef.value).clone())
        }
        ScopeId::Block(block_id) => Some(db.block(block_id).get(typedef.value).clone()),
        ScopeId::Subroutine(subroutine_id) => {
            Some(db.subroutine(subroutine_id).get(typedef.value).clone())
        }
    }
}

fn struct_of(
    db: &dyn HirDb,
    struct_id: InContainer<StructId>,
) -> Option<crate::hir_def::aggregate::StructDef> {
    match struct_id.cont_id {
        ScopeId::File(file_id) => Some(db.hir_file(file_id).get(struct_id.value).clone()),
        ScopeId::Module(module_id) => Some(db.module(module_id).get(struct_id.value).clone()),
        ScopeId::GenerateBlock(generate_block_id) => {
            Some(db.generate_block(generate_block_id).get(struct_id.value).clone())
        }
        ScopeId::Block(block_id) => Some(db.block(block_id).get(struct_id.value).clone()),
        ScopeId::Subroutine(subroutine_id) => {
            Some(db.subroutine(subroutine_id).get(struct_id.value).clone())
        }
    }
}

fn stmt_of(
    db: &dyn HirDb,
    stmt: InContainer<crate::hir_def::stmt::StmtId>,
) -> Option<crate::hir_def::stmt::Stmt> {
    match stmt.cont_id {
        ScopeId::File(file_id) => Some(db.hir_file(file_id).get(stmt.value).clone()),
        ScopeId::Module(module_id) => Some(db.module(module_id).get(stmt.value).clone()),
        ScopeId::GenerateBlock(generate_block_id) => {
            Some(db.generate_block(generate_block_id).get(stmt.value).clone())
        }
        ScopeId::Block(block_id) => Some(db.block(block_id).get(stmt.value).clone()),
        ScopeId::Subroutine(subroutine_id) => {
            Some(db.subroutine(subroutine_id).get(stmt.value).clone())
        }
    }
}
