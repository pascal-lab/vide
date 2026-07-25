use hir_def::{
    Ident,
    aggregate::{StructId, StructKind},
    container::{ArenaOwnerId, InContainer, InSubroutine},
    declaration::Declaration,
    def_id::DefId,
    expr::{
        BinaryOp, Expr, ExprId, UnaryOp,
        data_ty::{BuiltinDataTy, DataTy, Dimension, IntKind, NamedDataTy},
        declarator::{DeclId, DeclaratorParent},
    },
    literal::Literal,
    module::{ModuleId, ModuleKind, generate::GenerateBlockId, port::PortDeclId},
    pathres::{instance_target_def_id, resolve_name},
    stmt::{ForInit, StmtKind},
    subroutine::SubroutinePortId,
    symbol::{DefKind, DefOriginLoc, NameContext, NameScope, Resolution},
    typedef::TypedefId,
};
use rustc_hash::FxHashSet;
use triomphe::Arc;
use utils::get::GetRef;

use crate::{
    Type, TypeDiagnostic,
    db::TyDb,
    ty::{BuiltinTy, Ty, TyClass, TyMember, TyResult},
};

pub(crate) fn normalize_data_ty(
    db: &dyn TyDb,
    container: ArenaOwnerId,
    data_ty: DataTy,
) -> TyResult {
    normalize_data_ty_with_owner(db, container, data_ty, None)
}

fn normalize_data_ty_with_owner(
    db: &dyn TyDb,
    container: ArenaOwnerId,
    data_ty: DataTy,
    owner: Option<DefId>,
) -> TyResult {
    normalize_data_ty_inner(db, container, data_ty, owner, &mut FxHashSet::default())
}

pub(crate) fn type_of_path_resolution_query(db: &dyn TyDb, res: Resolution<DefId>) -> Arc<Type> {
    Arc::new(type_of_path_resolution_impl(db, res).into())
}

pub(crate) fn type_of_expr_query(db: &dyn TyDb, expr: InContainer<ExprId>) -> Arc<Type> {
    Arc::new(type_of_expr_impl(db, expr).into())
}

fn type_of_typedef_impl(db: &dyn TyDb, typedef: InContainer<TypedefId>) -> TyResult {
    type_of_typedef_inner(db, typedef, &mut FxHashSet::default())
}

fn type_of_decl_impl(db: &dyn TyDb, decl: InContainer<DeclId>) -> TyResult {
    let Some(data_ty) = data_ty_of_decl(db, decl) else {
        return TyResult::new(Ty::Unknown);
    };
    let owner = DefId::new(db, decl);
    let mut result = normalize_data_ty_with_owner(db, decl.cont_id, data_ty, Some(owner));
    if let Some(declarator) = decl_of(db, decl) {
        result.ty = apply_unpacked_dimensions(db, decl.cont_id, result.ty, &declarator.dimensions);
    }
    result
}

fn type_of_path_resolution_impl(db: &dyn TyDb, res: Resolution<DefId>) -> TyResult {
    res.unique()
        .map(|def_id| type_of_def_id(db, def_id))
        .unwrap_or_else(|| TyResult::new(Ty::Unknown))
}

fn type_of_def_id(db: &dyn TyDb, def_id: DefId) -> TyResult {
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
            .and_then(|instance| instance_target_def_id(db, instance.module_id, instance.value))
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
                | DefKind::Cross
                | DefKind::Stmt => TyResult::new(Ty::Unknown),
            })
            .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
        DefKind::Modport => origin
            .as_modport(db)
            .map(|modport| {
                TyResult::new(Ty::VirtualInterface {
                    def: DefId::new(db, modport.module_id),
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
            | DefKind::Covergroup
            | DefKind::Coverpoint
            | DefKind::Cross
            | DefKind::Stmt => {}
        }
    }
    port_ty.unwrap_or_else(|| TyResult::new(Ty::Unknown))
}

fn type_of_expr_impl(db: &dyn TyDb, expr: InContainer<ExprId>) -> TyResult {
    let Some(hir_expr) = expr_of(db, expr) else {
        return TyResult::new(Ty::Unknown);
    };

    match hir_expr {
        Expr::Ident(ident) => type_of_path_resolution_impl(
            db,
            resolve_name(db, expr.cont_id.into(), &ident, NameContext::Value),
        ),
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

pub(crate) fn members_of_ty(db: &dyn TyDb, ty: &Ty) -> Vec<TyMember> {
    match ty {
        Ty::Alias { target, .. } => members_of_ty(db, target),
        Ty::Struct(struct_id) => struct_members(db, *struct_id),
        Ty::Union(def_id) => union_members(db, *def_id),
        Ty::Module(module_id) => module_members(db, *module_id),
        Ty::Checker(def_id) => checker_members(db, *def_id),
        Ty::Covergroup(def_id) => covergroup_members(db, *def_id),
        Ty::VirtualInterface { def, .. } => def
            .primary_origin(db)
            .as_module(db)
            .map(|module_id| module_members(db, module_id))
            .unwrap_or_default(),
        Ty::GenerateBlock(generate_block_id) => generate_block_members(db, *generate_block_id),
        Ty::Block(block_id) => block_members(db, *block_id),
        Ty::Unknown
        | Ty::Error
        | Ty::Void
        | Ty::Builtin(_)
        | Ty::Enum(_)
        | Ty::Queue { .. }
        | Ty::Assoc { .. }
        | Ty::Dynamic(_)
        | Ty::Event
        | Ty::Chandle => Vec::new(),
    }
}

fn select_member(db: &dyn TyDb, base: &Ty, name: &Ident) -> TyResult {
    members_of_ty(db, base)
        .into_iter()
        .find(|member| &member.name == name)
        .map(|member| TyResult::new(member.ty))
        .unwrap_or_else(|| TyResult::new(Ty::Unknown))
}

pub(crate) fn type_class(db: &dyn TyDb, ty: &Ty) -> Option<TyClass> {
    match ty {
        Ty::Alias { target, .. } => type_class(db, target),
        Ty::Builtin(BuiltinTy::Data { id, .. }) => match db.lookup_intern_ty(*id) {
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

pub(crate) fn packed_bit_width(db: &dyn TyDb, ty: &Ty) -> Option<u64> {
    match ty {
        Ty::Alias { target, .. } => packed_bit_width(db, target),
        Ty::Builtin(BuiltinTy::Data { id, container }) => match db.lookup_intern_ty(*id) {
            BuiltinDataTy::String
            | BuiltinDataTy::Real(_)
            | BuiltinDataTy::Event
            | BuiltinDataTy::Chandle
            | BuiltinDataTy::Void => None,
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

fn normalize_data_ty_inner(
    db: &dyn TyDb,
    container: ArenaOwnerId,
    data_ty: DataTy,
    owner: Option<DefId>,
    seen: &mut FxHashSet<InContainer<TypedefId>>,
) -> TyResult {
    match data_ty {
        DataTy::Builtin(builtin) => match db.lookup_intern_ty(builtin) {
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
        DataTy::Enum => {
            owner.map(Ty::Enum).map(TyResult::new).unwrap_or_else(|| TyResult::new(Ty::Unknown))
        }
        DataTy::Named(named) => type_of_named_data_ty(db, container, named, seen),
    }
}

fn type_of_named_data_ty(
    db: &dyn TyDb,
    container: ArenaOwnerId,
    named: NamedDataTy,
    seen: &mut FxHashSet<InContainer<TypedefId>>,
) -> TyResult {
    let expr_id = match named {
        NamedDataTy::Ident(expr_id) | NamedDataTy::Field(expr_id) => expr_id,
    };
    let Some(Expr::Ident(ident)) = expr_of(db, InContainer::new(container, expr_id)) else {
        return TyResult::new(Ty::Unknown);
    };

    let resolution = resolve_name(db, container.into(), &ident, NameContext::Type);
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
    typedef: InContainer<TypedefId>,
    seen: &mut FxHashSet<InContainer<TypedefId>>,
) -> TyResult {
    if !seen.insert(typedef) {
        return TyResult {
            ty: Ty::Error,
            diagnostics: vec![TypeDiagnostic::TypedefCycle(typedef)],
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

    let owner = DefId::new(db, typedef);
    let mut target = normalize_data_ty_inner(db, typedef.cont_id, data_ty, Some(owner), seen);
    seen.remove(&typedef);
    let ty = if matches!(target.ty, Ty::Error) {
        Ty::Error
    } else {
        Ty::Alias { typedef, target: Box::new(target.ty) }
    };
    TyResult { ty, diagnostics: std::mem::take(&mut target.diagnostics) }
}

fn struct_members(db: &dyn TyDb, struct_id: InContainer<StructId>) -> Vec<TyMember> {
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
            Some(TyMember { name, ty })
        })
        .collect()
}

fn union_members(db: &dyn TyDb, def_id: DefId) -> Vec<TyMember> {
    aggregate_struct_id_from_def(db, def_id)
        .filter(|struct_id| struct_kind(db, *struct_id) == Some(StructKind::Union))
        .map(|struct_id| struct_members(db, struct_id))
        .unwrap_or_default()
}

fn aggregate_struct_id_from_def(db: &dyn TyDb, def_id: DefId) -> Option<InContainer<StructId>> {
    match def_id.primary_origin(db).loc(db) {
        DefOriginLoc::Typedef(typedef) => match typedef_of(db, typedef)?.ty? {
            DataTy::Struct(struct_id) => Some(struct_id),
            _ => None,
        },
        DefOriginLoc::Decl(decl) => match data_ty_of_decl(db, decl)? {
            DataTy::Struct(struct_id) => Some(struct_id),
            _ => None,
        },
        _ => None,
    }
}

fn struct_kind(db: &dyn TyDb, struct_id: InContainer<StructId>) -> Option<StructKind> {
    struct_of(db, struct_id).map(|def| def.kind)
}

fn apply_unpacked_dimensions(
    db: &dyn TyDb,
    container: ArenaOwnerId,
    mut ty: Ty,
    dimensions: &[Option<Dimension>],
) -> Ty {
    for dim in dimensions.iter().flatten() {
        ty = match dim {
            Dimension::Queue(size) => Ty::Queue { elem: Box::new(ty), size: *size },
            Dimension::Assoc(key) => Ty::Assoc {
                key: Box::new(type_of_dimension_key(db, container, *key)),
                elem: Box::new(ty),
            },
            Dimension::Dynamic => Ty::Dynamic(Box::new(ty)),
            Dimension::Size(key) if builtin_dimension_key_ty(db, container, *key).is_some() => {
                Ty::Assoc {
                    key: Box::new(type_of_dimension_key(db, container, *key)),
                    elem: Box::new(ty),
                }
            }
            Dimension::Range(_, _) | Dimension::Size(_) => ty,
        };
    }
    ty
}

fn type_of_dimension_key(db: &dyn TyDb, container: ArenaOwnerId, expr_id: ExprId) -> Ty {
    if let Some(ty) = builtin_dimension_key_ty(db, container, expr_id) {
        return ty;
    }
    type_of_expr_impl(db, InContainer::new(container, expr_id)).ty
}

fn builtin_dimension_key_ty(db: &dyn TyDb, container: ArenaOwnerId, expr_id: ExprId) -> Option<Ty> {
    if let Some(Expr::Ident(ident)) = expr_of(db, InContainer::new(container, expr_id)) {
        return builtin_type_name_ty(db, container, &ident);
    }
    None
}

fn builtin_type_name_ty(db: &dyn TyDb, container: ArenaOwnerId, ident: &Ident) -> Option<Ty> {
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
    Some(Ty::Builtin(BuiltinTy::Data { id: db.intern_ty(ty), container }))
}

fn module_members(db: &dyn TyDb, module_id: ModuleId) -> Vec<TyMember> {
    let file = db.hir_file(module_id.file_id);
    let scope = if file.get(module_id.value).kind == ModuleKind::Package {
        db.package_export_scope(module_id)
    } else {
        db.module_scope(module_id)
    };

    let mut members: Vec<_> = scope
        .iter_listing()
        .map(|(name, defs)| {
            let resolution = Resolution::from_candidates(defs);
            let ty = type_of_path_resolution_impl(db, resolution).ty;
            TyMember { name: name.clone(), ty }
        })
        .collect();
    sort_members(&mut members);
    members
}

fn checker_members(db: &dyn TyDb, def_id: DefId) -> Vec<TyMember> {
    let Some(checker_id) = def_id.primary_origin(db).as_checker(db) else {
        return Vec::new();
    };
    scope_members(db, &db.checker_scope(checker_id))
}

fn covergroup_members(db: &dyn TyDb, def_id: DefId) -> Vec<TyMember> {
    let Some(covergroup_id) = def_id.primary_origin(db).as_covergroup(db) else {
        return Vec::new();
    };
    scope_members(db, &db.covergroup_scope(covergroup_id))
}

fn generate_block_members(db: &dyn TyDb, generate_block_id: GenerateBlockId) -> Vec<TyMember> {
    scope_members(db, &db.generate_block_scope(generate_block_id))
}

fn block_members(db: &dyn TyDb, block_id: hir_def::block::BlockId) -> Vec<TyMember> {
    scope_members(db, &db.block_scope(block_id))
}

fn scope_members(db: &dyn TyDb, scope: &NameScope) -> Vec<TyMember> {
    let mut members: Vec<_> = scope
        .iter_listing()
        .map(|(name, defs)| {
            let resolution = Resolution::from_candidates(defs);
            let ty = type_of_path_resolution_impl(db, resolution).ty;
            TyMember { name: name.clone(), ty }
        })
        .collect();
    sort_members(&mut members);
    members
}

fn sort_members(members: &mut Vec<TyMember>) {
    members.sort_by(|left, right| left.name.cmp(&right.name));
    members.dedup_by(|left, right| left.name == right.name);
}

fn data_ty_of_decl(db: &dyn TyDb, decl: InContainer<DeclId>) -> Option<DataTy> {
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

fn port_decl_ty(db: &dyn TyDb, cont_id: ArenaOwnerId, port_decl_id: PortDeclId) -> Option<DataTy> {
    let ArenaOwnerId::Module(module_id) = cont_id else {
        return None;
    };
    let module = db.module(module_id);
    Some(module.ports.get(port_decl_id).header.ty())
}

fn for_init_decl_ty(
    db: &dyn TyDb,
    cont_id: ArenaOwnerId,
    stmt_id: hir_def::stmt::StmtId,
    decl_id: DeclId,
) -> Option<DataTy> {
    let stmt = stmt_of(db, InContainer::new(cont_id, stmt_id))?;
    let StmtKind::For { inits: ForInit::Init(inits), .. } = &stmt.kind else {
        return None;
    };
    inits.iter().find_map(|(ty, decl)| (*decl == decl_id).then_some(*ty).flatten())
}

fn type_of_subroutine_port_impl(db: &dyn TyDb, port: InSubroutine<SubroutinePortId>) -> TyResult {
    let subroutine = db.subroutine(port.subroutine);
    let port_id = port;
    let Some(port) = subroutine.ports.get(port_id.value.0 as usize) else {
        return TyResult::new(Ty::Unknown);
    };
    port.ty
        .map(|ty| {
            normalize_data_ty_with_owner(
                db,
                port_id.subroutine.into(),
                ty,
                Some(DefId::new(db, port_id)),
            )
        })
        .unwrap_or_else(|| TyResult::new(Ty::Unknown))
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

fn eval_const_i128(db: &dyn TyDb, container: ArenaOwnerId, expr_id: ExprId) -> Option<i128> {
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

fn expr_of(db: &dyn TyDb, expr: InContainer<ExprId>) -> Option<Expr> {
    match expr.cont_id {
        ArenaOwnerId::File(file_id) => Some(db.hir_file(file_id).get(expr.value).clone()),
        ArenaOwnerId::Module(module_id) => Some(db.module(module_id).get(expr.value).clone()),
        ArenaOwnerId::GenerateBlock(generate_block_id) => {
            Some(db.generate_block(generate_block_id).get(expr.value).clone())
        }
        ArenaOwnerId::Block(block_id) => Some(db.block(block_id).get(expr.value).clone()),
        ArenaOwnerId::Subroutine(subroutine_id) => {
            Some(db.subroutine(subroutine_id).get(expr.value).clone())
        }
    }
}

fn decl_of(
    db: &dyn TyDb,
    decl: InContainer<DeclId>,
) -> Option<hir_def::expr::declarator::Declarator> {
    match decl.cont_id {
        ArenaOwnerId::File(file_id) => Some(db.hir_file(file_id).get(decl.value).clone()),
        ArenaOwnerId::Module(module_id) => Some(db.module(module_id).get(decl.value).clone()),
        ArenaOwnerId::GenerateBlock(generate_block_id) => {
            Some(db.generate_block(generate_block_id).get(decl.value).clone())
        }
        ArenaOwnerId::Block(block_id) => Some(db.block(block_id).get(decl.value).clone()),
        ArenaOwnerId::Subroutine(subroutine_id) => {
            Some(db.subroutine(subroutine_id).get(decl.value).clone())
        }
    }
}

fn declaration_of(
    db: &dyn TyDb,
    decl: InContainer<hir_def::declaration::DeclarationId>,
) -> Option<Declaration> {
    match decl.cont_id {
        ArenaOwnerId::File(file_id) => Some(db.hir_file(file_id).get(decl.value).clone()),
        ArenaOwnerId::Module(module_id) => Some(db.module(module_id).get(decl.value).clone()),
        ArenaOwnerId::GenerateBlock(generate_block_id) => {
            Some(db.generate_block(generate_block_id).get(decl.value).clone())
        }
        ArenaOwnerId::Block(block_id) => Some(db.block(block_id).get(decl.value).clone()),
        ArenaOwnerId::Subroutine(subroutine_id) => {
            Some(db.subroutine(subroutine_id).get(decl.value).clone())
        }
    }
}

fn typedef_of(db: &dyn TyDb, typedef: InContainer<TypedefId>) -> Option<hir_def::typedef::Typedef> {
    match typedef.cont_id {
        ArenaOwnerId::File(file_id) => Some(db.hir_file(file_id).get(typedef.value).clone()),
        ArenaOwnerId::Module(module_id) => Some(db.module(module_id).get(typedef.value).clone()),
        ArenaOwnerId::GenerateBlock(generate_block_id) => {
            Some(db.generate_block(generate_block_id).get(typedef.value).clone())
        }
        ArenaOwnerId::Block(block_id) => Some(db.block(block_id).get(typedef.value).clone()),
        ArenaOwnerId::Subroutine(subroutine_id) => {
            Some(db.subroutine(subroutine_id).get(typedef.value).clone())
        }
    }
}

fn struct_of(
    db: &dyn TyDb,
    struct_id: InContainer<StructId>,
) -> Option<hir_def::aggregate::StructDef> {
    match struct_id.cont_id {
        ArenaOwnerId::File(file_id) => Some(db.hir_file(file_id).get(struct_id.value).clone()),
        ArenaOwnerId::Module(module_id) => Some(db.module(module_id).get(struct_id.value).clone()),
        ArenaOwnerId::GenerateBlock(generate_block_id) => {
            Some(db.generate_block(generate_block_id).get(struct_id.value).clone())
        }
        ArenaOwnerId::Block(block_id) => Some(db.block(block_id).get(struct_id.value).clone()),
        ArenaOwnerId::Subroutine(subroutine_id) => {
            Some(db.subroutine(subroutine_id).get(struct_id.value).clone())
        }
    }
}

fn stmt_of(db: &dyn TyDb, stmt: InContainer<hir_def::stmt::StmtId>) -> Option<hir_def::stmt::Stmt> {
    match stmt.cont_id {
        ArenaOwnerId::File(file_id) => Some(db.hir_file(file_id).get(stmt.value).clone()),
        ArenaOwnerId::Module(module_id) => Some(db.module(module_id).get(stmt.value).clone()),
        ArenaOwnerId::GenerateBlock(generate_block_id) => {
            Some(db.generate_block(generate_block_id).get(stmt.value).clone())
        }
        ArenaOwnerId::Block(block_id) => Some(db.block(block_id).get(stmt.value).clone()),
        ArenaOwnerId::Subroutine(subroutine_id) => {
            Some(db.subroutine(subroutine_id).get(stmt.value).clone())
        }
    }
}
