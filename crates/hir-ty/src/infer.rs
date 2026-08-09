use hir_def::{
    Ident,
    aggregate::{StructId, StructKind},
    container::OwnerRef,
    def_id::DefId,
    expr::{
        Expr, ExprId,
        data_ty::{BuiltinDataTy, BuiltinDataTyId, DataTy, Dimension, IntKind, TypeRef},
        declarator::{DeclId, DeclaratorParent},
    },
    module::port::PortDeclId,
    owner::OwnerId,
    pathres::{NameRef, RefKind, instance_target_def_id, resolve_name_at, resolve_path},
    stmt::{ForInit, StmtKind},
    subroutine::SubroutinePortId,
    symbol::{DefKind, NameContext, Resolution},
    typedef::TypedefId,
};
use rustc_hash::FxHashSet;
use utils::get::GetRef;

use crate::{
    Type, TypeDiagnostic,
    db::TyDb,
    members::select_member,
    ty::{BuiltinTy, Ty, TyResult},
};

#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
pub(crate) struct ExprQueryKey {
    #[returns(copy)]
    pub owner: hir_def::owner::OwnerId,
    #[returns(copy)]
    pub local: u32,
}
pub(crate) fn normalize_data_ty(db: &dyn TyDb, container: OwnerId, data_ty: DataTy) -> TyResult {
    normalize_data_ty_with_owner(db, container, data_ty, None)
}

fn normalize_data_ty_with_owner(
    db: &dyn TyDb,
    container: OwnerId,
    data_ty: DataTy,
    owner: Option<DefId>,
) -> TyResult {
    normalize_data_ty_inner(db, container, data_ty, owner, &mut FxHashSet::default())
}

#[salsa::tracked(returns(clone))]
pub(crate) fn type_of_def_origin_query(db: &dyn TyDb, origin: hir_def::symbol::DefOrigin) -> Type {
    let def_id = DefId::from_origin(db, origin);
    type_of_def_id(db, def_id).into()
}

#[salsa::tracked(returns(clone))]
pub(crate) fn type_of_expr_query(db: &dyn TyDb, key: ExprQueryKey) -> Type {
    let body = db.body_with_source_map(key.owner(db));
    let (expr_id, _) = body
        .exprs
        .iter()
        .nth(key.local(db) as usize)
        .expect("expression query key must refer to an expression in its owner body");
    let expr = OwnerRef::new(key.owner(db), expr_id);
    type_of_expr_impl(db, expr).into()
}
fn type_of_typedef_impl(db: &dyn TyDb, typedef: OwnerRef<TypedefId>) -> TyResult {
    type_of_typedef_inner(db, typedef, &mut FxHashSet::default())
}

fn type_of_decl_impl(db: &dyn TyDb, decl: OwnerRef<DeclId>) -> TyResult {
    let Some(data_ty) = data_ty_of_decl(db, decl) else {
        return TyResult::new(Ty::Unknown);
    };
    let owner = DefId::from_source(db, decl);
    let mut result = normalize_data_ty_with_owner(db, decl.cont_id, data_ty, Some(owner));
    let data = decl.cont_id.data(db);
    result.ty = apply_unpacked_dimensions(
        db,
        decl.cont_id,
        result.ty,
        &data.declarator(decl.value).dimensions,
    );
    result
}

pub(crate) fn type_of_path_resolution_impl(db: &dyn TyDb, res: Resolution<DefId>) -> TyResult {
    res.unique()
        .map(|def_id| type_of_def_id(db, def_id))
        .unwrap_or_else(|| TyResult::new(Ty::Unknown))
}
pub(crate) fn type_of_def_id(db: &dyn TyDb, def_id: DefId) -> TyResult {
    if def_id.is_non_ansi_port(db) {
        return type_of_non_ansi_port(db, def_id);
    }
    let origin = def_id.primary_origin(db);
    match def_id.kind(db) {
        DefKind::Module | DefKind::Package | DefKind::Program => origin
            .as_module(db)
            .map(|module_id| TyResult::new(Ty::Module(module_id)))
            .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
        DefKind::Interface => TyResult::new(Ty::VirtualInterface { def: def_id, modport: None }),
        DefKind::Checker => TyResult::new(Ty::Checker(def_id)),
        DefKind::Covergroup => TyResult::new(Ty::Covergroup(def_id)),
        DefKind::Port
        | DefKind::CheckerPort
        | DefKind::Variable
        | DefKind::Net
        | DefKind::Param
        | DefKind::Genvar
        | DefKind::Specparam => origin
            .as_decl(db)
            .map(|decl| type_of_decl_impl(db, decl))
            .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
        DefKind::Typedef => origin
            .as_typedef(db)
            .map(|typedef| type_of_typedef_impl(db, typedef))
            .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
        DefKind::SubroutinePort => origin
            .as_subroutine_port(db)
            .map(|port| type_of_subroutine_port_impl(db, port))
            .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
        DefKind::Instance => origin
            .as_instance(db)
            .and_then(|instance| instance_target_def_id(db, instance.cont_id, instance.value))
            .map(|target| match target.kind(db) {
                DefKind::Interface => {
                    TyResult::new(Ty::VirtualInterface { def: target, modport: None })
                }
                DefKind::Module | DefKind::Program => target
                    .primary_origin(db)
                    .as_module(db)
                    .map(|module_id| TyResult::new(Ty::Module(module_id)))
                    .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
                DefKind::Checker => TyResult::new(Ty::Checker(target)),
                DefKind::Covergroup => TyResult::new(Ty::Covergroup(target)),
                DefKind::Package
                | DefKind::Udp
                | DefKind::Config
                | DefKind::Library
                | DefKind::Block
                | DefKind::GenerateBlock
                | DefKind::Subroutine
                | DefKind::SubroutinePort
                | DefKind::NonAnsiPort
                | DefKind::Typedef
                | DefKind::Net
                | DefKind::Variable
                | DefKind::Param
                | DefKind::Port
                | DefKind::Genvar
                | DefKind::Specparam
                | DefKind::Instance
                | DefKind::Modport
                | DefKind::ClockingBlock
                | DefKind::ClockingSignal
                | DefKind::CheckerPort
                | DefKind::Coverpoint
                | DefKind::Property
                | DefKind::Sequence
                | DefKind::Cross
                | DefKind::Stmt => TyResult::new(Ty::Unknown),
            })
            .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
        DefKind::Modport => origin
            .as_modport(db)
            .map(|modport| {
                TyResult::new(Ty::VirtualInterface {
                    def: DefId::from_owner(db, modport.cont_id)
                        .expect("modport container must have a module definition"),
                    modport: Some(def_id),
                })
            })
            .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
        DefKind::GenerateBlock => origin
            .as_generate_block(db)
            .map(|generate_block_id| TyResult::new(Ty::GenerateBlock(generate_block_id)))
            .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
        DefKind::Block => origin
            .as_block(db)
            .map(|block_id| TyResult::new(Ty::Block(block_id)))
            .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
        DefKind::Udp
        | DefKind::Config
        | DefKind::Library
        | DefKind::Subroutine
        | DefKind::NonAnsiPort
        | DefKind::ClockingBlock
        | DefKind::ClockingSignal
        | DefKind::Property
        | DefKind::Sequence
        | DefKind::Coverpoint
        | DefKind::Cross
        | DefKind::Stmt => TyResult::new(Ty::Unknown),
    }
}
fn type_of_non_ansi_port(db: &dyn TyDb, def_id: DefId) -> TyResult {
    let mut port_ty = None;
    for origin in def_id.origins(db) {
        let Some(decl) = origin.as_decl(db) else {
            continue;
        };
        let ty = type_of_decl_impl(db, decl);
        match origin.kind(db) {
            DefKind::Variable | DefKind::Net if !matches!(ty.ty, Ty::Unknown) => return ty,
            DefKind::Port => {
                port_ty.get_or_insert(ty);
            }
            DefKind::Variable
            | DefKind::Net
            | DefKind::Module
            | DefKind::Interface
            | DefKind::Package
            | DefKind::Program
            | DefKind::Udp
            | DefKind::Config
            | DefKind::Library
            | DefKind::Block
            | DefKind::GenerateBlock
            | DefKind::Subroutine
            | DefKind::SubroutinePort
            | DefKind::NonAnsiPort
            | DefKind::Typedef
            | DefKind::Param
            | DefKind::Genvar
            | DefKind::Specparam
            | DefKind::Instance
            | DefKind::Modport
            | DefKind::ClockingBlock
            | DefKind::ClockingSignal
            | DefKind::Checker
            | DefKind::CheckerPort
            | DefKind::Property
            | DefKind::Sequence
            | DefKind::Covergroup
            | DefKind::Coverpoint
            | DefKind::Cross
            | DefKind::Stmt => {}
        }
    }
    port_ty.unwrap_or_else(|| TyResult::new(Ty::Unknown))
}

fn type_of_expr_impl(db: &dyn TyDb, expr: OwnerRef<ExprId>) -> TyResult {
    let data = expr.cont_id.data(db);
    match data.expr(expr.value) {
        Expr::Ident(ident) => {
            // Expression references resolve at their source position so a
            // later declaration never shadows an import (26.3).
            let reference = expr_reference(db, expr);
            type_of_path_resolution_impl(
                db,
                resolve_name_at(db, expr.cont_id, ident, NameContext::Value, reference.as_ref()),
            )
        }
        Expr::Field { receiver, field } => {
            let Some(field) = field else {
                return TyResult::new(Ty::Unknown);
            };
            let base = type_of_expr_impl(db, expr.with_value(*receiver));
            if matches!(base.ty, Ty::Unknown | Ty::Error) {
                return base;
            }
            let mut selected = select_member(db, &base.ty, field);
            selected.diagnostics.extend(base.diagnostics);
            selected
        }
        Expr::ElementSelect { receiver, .. } => type_of_expr_impl(db, expr.with_value(*receiver)),
        Expr::Cast { ty, .. } => normalize_data_ty(db, expr.cont_id, ty.clone()),
        _ => TyResult::new(Ty::Unknown),
    }
}

/// Reference position of an expression, derived from its canonical source.
fn expr_reference(db: &dyn TyDb, expr: OwnerRef<ExprId>) -> Option<NameRef> {
    let file_id = expr.cont_id.file(db);
    let source =
        db.body_with_source_map(expr.cont_id).source_map().expr_srcs.hir_to_src(expr.value)?;
    Some(NameRef {
        position: hir_def::container::InFile::new(file_id, source),
        kind: RefKind::Value,
    })
}

fn normalize_data_ty_inner(
    db: &dyn TyDb,
    container: OwnerId,
    data_ty: DataTy,
    owner: Option<DefId>,
    seen: &mut FxHashSet<OwnerRef<TypedefId>>,
) -> TyResult {
    match data_ty {
        DataTy::Builtin(builtin) => match builtin.get() {
            BuiltinDataTy::Void => TyResult::new(Ty::Void),
            BuiltinDataTy::Event => TyResult::new(Ty::Event),
            BuiltinDataTy::Chandle => TyResult::new(Ty::Chandle),
            _ => TyResult::new(Ty::Builtin(BuiltinTy::Data { id: builtin, container })),
        },
        DataTy::Struct(struct_id) => match struct_kind(db, struct_id) {
            Some(StructKind::Union) => owner
                .map(Ty::Union)
                .map(TyResult::new)
                .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
            Some(StructKind::Struct) | None => TyResult::new(Ty::Struct(struct_id)),
        },
        DataTy::Named(named) => type_of_named_data_ty(db, container, named, seen),
        DataTy::Enum => {
            owner.map(Ty::Enum).map(TyResult::new).unwrap_or_else(|| TyResult::new(Ty::Unknown))
        }
        DataTy::Unsupported(kind) => {
            TyResult { ty: Ty::Error, diagnostics: vec![TypeDiagnostic::UnsupportedDataType(kind)] }
        }
    }
}

fn type_of_named_data_ty(
    db: &dyn TyDb,
    container: OwnerId,
    named: TypeRef,
    seen: &mut FxHashSet<OwnerRef<TypedefId>>,
) -> TyResult {
    if let Some(recovery) = named.recovery() {
        return TyResult {
            ty: Ty::Error,
            diagnostics: vec![TypeDiagnostic::InvalidTypePath(recovery)],
        };
    }
    let resolution = resolve_path(db, container, named.segments(), NameContext::Type);
    let Some(def_id) = resolution.unique() else {
        return TyResult::new(Ty::Unknown);
    };
    if let Some(typedef) = def_id.primary_origin(db).as_typedef(db) {
        return type_of_typedef_inner(db, typedef, seen);
    }
    type_of_def_id(db, def_id)
}

fn type_of_typedef_inner(
    db: &dyn TyDb,
    typedef: OwnerRef<TypedefId>,
    seen: &mut FxHashSet<OwnerRef<TypedefId>>,
) -> TyResult {
    if !seen.insert(typedef) {
        return TyResult {
            ty: Ty::Error,
            diagnostics: vec![TypeDiagnostic::TypedefCycle(typedef)],
        };
    }

    let data = typedef.cont_id.data(db);
    let Some(data_ty) = data.typedef(typedef.value).ty.clone() else {
        seen.remove(&typedef);
        return TyResult::new(Ty::Unknown);
    };

    let owner = DefId::from_source(db, typedef);
    let mut target = normalize_data_ty_inner(db, typedef.cont_id, data_ty, Some(owner), seen);
    seen.remove(&typedef);
    let ty = if matches!(target.ty, Ty::Error) {
        Ty::Error
    } else {
        Ty::Alias { typedef, target: Box::new(target.ty) }
    };
    TyResult { ty, diagnostics: std::mem::take(&mut target.diagnostics) }
}

fn struct_kind(db: &dyn TyDb, struct_id: OwnerRef<StructId>) -> Option<StructKind> {
    Some(struct_id.cont_id.data(db).struct_def(struct_id.value).kind)
}

fn apply_unpacked_dimensions(
    db: &dyn TyDb,
    container: OwnerId,
    mut ty: Ty,
    dimensions: &[Option<Dimension>],
) -> Ty {
    for dim in dimensions.iter().flatten() {
        ty = match dim {
            Dimension::Queue(size) => Ty::Queue { elem: Box::new(ty), size: *size },
            Dimension::Assoc(key) => Ty::Assoc {
                key: Box::new(type_of_dimension_key(db, &container, *key)),
                elem: Box::new(ty),
            },
            Dimension::Dynamic => Ty::Dynamic(Box::new(ty)),
            Dimension::Size(key) if builtin_dimension_key_ty(db, &container, *key).is_some() => {
                Ty::Assoc {
                    key: Box::new(type_of_dimension_key(db, &container, *key)),
                    elem: Box::new(ty),
                }
            }
            Dimension::Range(_, _) | Dimension::Size(_) => ty,
        };
    }
    ty
}

fn type_of_dimension_key(db: &dyn TyDb, container: &OwnerId, expr_id: ExprId) -> Ty {
    if let Some(ty) = builtin_dimension_key_ty(db, container, expr_id) {
        return ty;
    }
    type_of_expr_impl(db, OwnerRef::new(*container, expr_id)).ty
}

fn builtin_dimension_key_ty(db: &dyn TyDb, container: &OwnerId, expr_id: ExprId) -> Option<Ty> {
    let data = container.data(db);
    if let Expr::Ident(ident) = data.expr(expr_id) {
        return builtin_type_name_ty(container, ident);
    }
    None
}

fn builtin_type_name_ty(container: &OwnerId, ident: &Ident) -> Option<Ty> {
    let ty = match ident.as_str() {
        "string" => BuiltinDataTy::String,
        "byte" => BuiltinDataTy::Int { kind: IntKind::Byte, signing: true },
        "shortint" => BuiltinDataTy::Int { kind: IntKind::ShortInt, signing: true },
        "int" => BuiltinDataTy::Int { kind: IntKind::Int, signing: true },
        "longint" => BuiltinDataTy::Int { kind: IntKind::LongInt, signing: true },
        "integer" => BuiltinDataTy::Int { kind: IntKind::Integer, signing: true },
        "time" => BuiltinDataTy::Int { kind: IntKind::Time, signing: false },
        "bit" => BuiltinDataTy::Vector {
            kind: hir_def::expr::data_ty::VecKind::Bit,
            signing: false,
            dimensions: Default::default(),
        },
        "logic" => BuiltinDataTy::default(),
        "reg" => BuiltinDataTy::Vector {
            kind: hir_def::expr::data_ty::VecKind::Reg,
            signing: false,
            dimensions: Default::default(),
        },
        _ => return None,
    };
    Some(Ty::Builtin(BuiltinTy::Data { id: BuiltinDataTyId::new(ty), container: *container }))
}

pub(crate) fn data_ty_of_decl(db: &dyn TyDb, decl: OwnerRef<DeclId>) -> Option<DataTy> {
    let data = decl.cont_id.data(db);
    match data.declarator(decl.value).parent {
        DeclaratorParent::DeclarationId(declaration_id) => {
            Some(data.declaration(declaration_id).ty())
        }
        DeclaratorParent::PortDeclId(port_decl_id) => port_decl_ty(db, decl.cont_id, port_decl_id),
        DeclaratorParent::StmtId(stmt_id) => {
            let StmtKind::For { inits: ForInit::Init(inits), .. } = &data.stmt(stmt_id).kind else {
                return None;
            };
            inits.iter().find_map(|(ty, candidate)| {
                (*candidate == decl.value).then_some(ty.clone()).flatten()
            })
        }
    }
}

fn port_decl_ty(db: &dyn TyDb, cont_id: OwnerId, port_decl_id: PortDeclId) -> Option<DataTy> {
    let module = db.body(cont_id);
    Some(module.ports.get(port_decl_id).header.ty())
}

fn type_of_subroutine_port_impl(db: &dyn TyDb, port: OwnerRef<SubroutinePortId>) -> TyResult {
    let owner = port.cont_id;
    let subroutine = db.subroutine(owner);
    let Some(port_data) = subroutine.ports.get(port.value.0 as usize) else {
        return TyResult::new(Ty::Unknown);
    };
    port_data
        .ty
        .clone()
        .map(|ty| normalize_data_ty_with_owner(db, owner, ty, Some(DefId::from_source(db, port))))
        .unwrap_or_else(|| TyResult::new(Ty::Unknown))
}
