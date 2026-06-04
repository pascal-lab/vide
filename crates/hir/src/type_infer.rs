use rustc_hash::FxHashSet;
use utils::get::GetRef;

use crate::{
    container::{
        ContainerId, ContainerParent, InContainer, InGenerateBlock, InModule, InSubroutine,
    },
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
        module::{
            ModuleId,
            generate::GenerateBlockId,
            port::{PortDeclId, PortHeader},
        },
        package_import::{PackageExportName, PackageImport, PackageImportName},
        stmt::{ForInit, StmtKind},
        subroutine::SubroutinePortId,
        typedef::TypedefId,
    },
    semantics::pathres::PathResolution,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinTy {
    Data { id: BuiltinDataTyId, container: ContainerId },
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

#[derive(Debug, Clone)]
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

pub fn normalize_data_ty(db: &dyn HirDb, container: ContainerId, data_ty: DataTy) -> TyResult {
    normalize_data_ty_inner(db, container, data_ty, &mut FxHashSet::default())
}

pub fn type_of_typedef(db: &dyn HirDb, typedef: InContainer<TypedefId>) -> TyResult {
    type_of_typedef_inner(db, typedef, &mut FxHashSet::default())
}

pub fn type_of_decl(db: &dyn HirDb, decl: InContainer<DeclId>) -> TyResult {
    if let Some(ty) = type_of_interface_port_decl(db, decl) {
        return ty;
    }

    let Some(data_ty) = data_ty_of_decl(db, decl) else {
        return TyResult::new(Ty::Unknown);
    };
    normalize_data_ty(db, decl.cont_id, data_ty)
}

pub fn type_of_path_resolution(db: &dyn HirDb, res: PathResolution) -> TyResult {
    match res {
        PathResolution::Module(module_id) => TyResult::new(Ty::Module(module_id)),
        PathResolution::Decl(decl) => type_of_decl(db, decl),
        PathResolution::Typedef(typedef) => type_of_typedef(db, typedef),
        PathResolution::ParamDecl(decl) | PathResolution::AnsiPort(decl) => {
            type_of_decl(db, decl.into())
        }
        PathResolution::NonAnsiPort { port_decl, data_decl, module, .. } => data_decl
            .or(port_decl)
            .map(|decl| type_of_decl(db, InContainer::new(module.into(), decl)))
            .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
        PathResolution::SubroutinePort(port) => type_of_subroutine_port(db, port),
        PathResolution::Instance(instance) => {
            instance_target_module_id(db, instance.module_id, instance.value)
                .map(|module_id| TyResult::new(Ty::Module(module_id)))
                .unwrap_or_else(|| TyResult::new(Ty::Unknown))
        }
        PathResolution::GenerateBlock(generate_block_id) => {
            TyResult::new(Ty::GenerateBlock(generate_block_id))
        }
        PathResolution::Modport(_) => TyResult::new(Ty::Unknown),
        PathResolution::Block(block_id) => TyResult::new(Ty::Block(block_id)),
        PathResolution::Config(_)
        | PathResolution::Library(_)
        | PathResolution::Udp(_)
        | PathResolution::Subroutine(_)
        | PathResolution::Stmt(_) => TyResult::new(Ty::Unknown),
    }
}

pub fn type_of_expr(db: &dyn HirDb, expr: InContainer<ExprId>) -> TyResult {
    let Some(hir_expr) = expr_of(db, expr) else {
        return TyResult::new(Ty::Unknown);
    };

    match hir_expr {
        Expr::Ident(ident) => resolve_name(db, expr.cont_id, &ident)
            .map(|res| type_of_path_resolution(db, res))
            .unwrap_or_else(|| TyResult::new(Ty::Unknown)),
        Expr::Field { receiver, field } => {
            let Some(field) = field else {
                return TyResult::new(Ty::Unknown);
            };
            let base = type_of_expr(db, expr.with_value(receiver));
            if matches!(base.ty, Ty::Unknown | Ty::Error) {
                return base;
            }
            let mut selected = select_member(db, &base.ty, &field);
            selected.diagnostics.extend(base.diagnostics);
            selected
        }
        Expr::ElementSelect { receiver, .. } => type_of_expr(db, expr.with_value(receiver)),
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
    container: ContainerId,
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
    container: ContainerId,
    named: NamedDataTy,
    seen: &mut FxHashSet<InContainer<TypedefId>>,
) -> TyResult {
    let expr_id = match named {
        NamedDataTy::Ident(expr_id) | NamedDataTy::Field(expr_id) => expr_id,
    };
    let Some(Expr::Ident(ident)) = expr_of(db, InContainer::new(container, expr_id)) else {
        return TyResult::new(Ty::Unknown);
    };

    match resolve_name(db, container, &ident) {
        Some(PathResolution::Typedef(typedef)) => type_of_typedef_inner(db, typedef, seen),
        Some(res) => type_of_path_resolution(db, res),
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
    let mut members: Vec<_> = db
        .module_scope(module_id)
        .iter()
        .map(|(name, entry)| {
            let origin = PathResolution::from(InModule::new(module_id, entry));
            let ty = type_of_path_resolution(db, origin).ty;
            TyMember { name: name.clone(), ty, origin: Some(origin) }
        })
        .collect();
    for name in exported_member_names(db, module_id, &mut FxHashSet::default()) {
        let Some(origin) =
            resolve_package_member_by_id(db, module_id, &name, &mut FxHashSet::default())
        else {
            continue;
        };
        let ty = type_of_path_resolution(db, origin).ty;
        members.push(TyMember { name, ty, origin: Some(origin) });
    }
    sort_members(&mut members);
    members
}

fn exported_member_names(
    db: &dyn HirDb,
    module_id: ModuleId,
    seen: &mut FxHashSet<ModuleId>,
) -> Vec<Ident> {
    if !seen.insert(module_id) {
        return Vec::new();
    }

    let module = db.module(module_id);
    let mut names = Vec::new();

    for (_, export) in module.package_exports.iter() {
        match &export.item {
            PackageExportName::Name(name) => {
                names.push(name.clone());
            }
            PackageExportName::Wildcard => {
                if let Some(package) = export.package.as_ref()
                    && let Some(target) = db.unit_scope().resolve_module(package).unique()
                {
                    names.extend(visible_module_member_names(db, target, seen));
                }
            }
            PackageExportName::AllImports => {
                for (_, import) in module.package_imports.iter() {
                    match &import.item {
                        PackageImportName::Name(name) => names.push(name.clone()),
                        PackageImportName::Wildcard => {
                            if let Some(package) = import.package.as_ref()
                                && let Some(target) =
                                    db.unit_scope().resolve_module(package).unique()
                            {
                                names.extend(visible_module_member_names(db, target, seen));
                            }
                        }
                    }
                }
            }
        }
    }

    seen.remove(&module_id);
    names
}

fn visible_module_member_names(
    db: &dyn HirDb,
    module_id: ModuleId,
    seen: &mut FxHashSet<ModuleId>,
) -> Vec<Ident> {
    db.module_scope(module_id)
        .iter()
        .map(|(name, _)| name.clone())
        .chain(exported_member_names(db, module_id, seen))
        .collect()
}

fn generate_block_members(db: &dyn HirDb, generate_block_id: GenerateBlockId) -> Vec<TyMember> {
    let mut members: Vec<_> = db
        .generate_block_scope(generate_block_id)
        .iter()
        .map(|(name, entry)| {
            let origin = PathResolution::from(InGenerateBlock::new(generate_block_id, entry));
            let ty = type_of_path_resolution(db, origin).ty;
            TyMember { name: name.clone(), ty, origin: Some(origin) }
        })
        .collect();
    sort_members(&mut members);
    members
}

fn block_members(db: &dyn HirDb, block_id: crate::hir_def::block::BlockId) -> Vec<TyMember> {
    let mut members: Vec<_> = db
        .block_scope(block_id)
        .iter()
        .map(|(name, entry)| {
            let origin = PathResolution::from(crate::container::InBlock::new(block_id, entry));
            let ty = type_of_path_resolution(db, origin).ty;
            TyMember { name: name.clone(), ty, origin: Some(origin) }
        })
        .collect();
    sort_members(&mut members);
    members
}

fn sort_members(members: &mut Vec<TyMember>) {
    members.sort_by(|left, right| left.name.cmp(&right.name));
    members.dedup_by(|left, right| left.name == right.name);
}

fn type_of_interface_port_decl(db: &dyn HirDb, decl: InContainer<DeclId>) -> Option<TyResult> {
    let declarator = decl_of(db, decl)?;
    let DeclaratorParent::PortDeclId(port_decl_id) = declarator.parent else {
        return None;
    };
    let ContainerId::ModuleId(module_id) = decl.cont_id else {
        return None;
    };
    let module = db.module(module_id);
    let PortHeader::Interface { interface: Some(interface), .. } =
        module.ports.get(port_decl_id).header.clone()
    else {
        return None;
    };
    let interface_id = db.unit_scope().resolve_module(&interface).unique()?;
    Some(TyResult::new(Ty::Module(interface_id)))
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

fn port_decl_ty(db: &dyn HirDb, cont_id: ContainerId, port_decl_id: PortDeclId) -> Option<DataTy> {
    let ContainerId::ModuleId(module_id) = cont_id else {
        return None;
    };
    let module = db.module(module_id);
    Some(module.ports.get(port_decl_id).header.ty())
}

fn for_init_decl_ty(
    db: &dyn HirDb,
    cont_id: ContainerId,
    stmt_id: crate::hir_def::stmt::StmtId,
    decl_id: DeclId,
) -> Option<DataTy> {
    let stmt = stmt_of(db, InContainer::new(cont_id, stmt_id))?;
    let StmtKind::For { inits: ForInit::Init(inits), .. } = &stmt.kind else {
        return None;
    };
    inits.iter().find_map(|(ty, decl)| (*decl == decl_id).then_some(*ty).flatten())
}

fn type_of_subroutine_port(db: &dyn HirDb, port: InSubroutine<SubroutinePortId>) -> TyResult {
    let subroutine = db.subroutine(port.subroutine);
    let port_id = port;
    let Some(port) = subroutine.ports.get(port_id.value.0 as usize) else {
        return TyResult::new(Ty::Unknown);
    };
    port.ty
        .map(|ty| normalize_data_ty(db, ContainerId::SubroutineId(port_id.subroutine), ty))
        .unwrap_or_else(|| TyResult::new(Ty::Unknown))
}

fn resolve_name(db: &dyn HirDb, cont_id: ContainerId, ident: &Ident) -> Option<PathResolution> {
    ContainerParent::start_from(db, cont_id).find_map(|id| {
        resolve_local_name(db, id, ident).or_else(|| resolve_imported_name(db, id, ident))
    })
}

fn resolve_local_name(
    db: &dyn HirDb,
    cont_id: ContainerId,
    ident: &Ident,
) -> Option<PathResolution> {
    match cont_id {
        ContainerId::HirFileId(_) => db.unit_scope().get(ident).map(PathResolution::from),
        ContainerId::ModuleId(module_id) => db
            .module_scope(module_id)
            .get(ident)
            .map(|entry| PathResolution::from(InModule::new(module_id, entry))),
        ContainerId::GenerateBlockId(generate_block_id) => db
            .generate_block_scope(generate_block_id)
            .get(ident)
            .map(|entry| PathResolution::from(InGenerateBlock::new(generate_block_id, entry))),
        ContainerId::BlockId(block_id) => db
            .block_scope(block_id)
            .get(ident)
            .map(|entry| PathResolution::from(crate::container::InBlock::new(block_id, entry))),
        ContainerId::SubroutineId(subroutine_id) => db
            .subroutine_scope(subroutine_id)
            .get(ident)
            .map(|entry| PathResolution::from(InSubroutine::new(subroutine_id, entry))),
    }
}

fn resolve_imported_name(
    db: &dyn HirDb,
    cont_id: ContainerId,
    ident: &Ident,
) -> Option<PathResolution> {
    match cont_id {
        ContainerId::HirFileId(file_id) => {
            let file = db.hir_file(file_id);
            resolve_imports(db, file.package_imports.iter().map(|(_, import)| import), ident)
        }
        ContainerId::ModuleId(module_id) => {
            let module = db.module(module_id);
            resolve_imports(db, module.package_imports.iter().map(|(_, import)| import), ident)
        }
        ContainerId::GenerateBlockId(generate_block_id) => {
            let generate_block = db.generate_block(generate_block_id);
            resolve_imports(
                db,
                generate_block.package_imports.iter().map(|(_, import)| import),
                ident,
            )
        }
        ContainerId::BlockId(block_id) => {
            let block = db.block(block_id);
            resolve_imports(db, block.package_imports.iter().map(|(_, import)| import), ident)
        }
        ContainerId::SubroutineId(subroutine_id) => {
            let subroutine = db.subroutine(subroutine_id);
            resolve_imports(db, subroutine.package_imports.iter().map(|(_, import)| import), ident)
        }
    }
}

fn resolve_imports<'a>(
    db: &dyn HirDb,
    imports: impl Iterator<Item = &'a PackageImport>,
    ident: &Ident,
) -> Option<PathResolution> {
    let mut explicit = Vec::new();
    let mut wildcard = Vec::new();

    for import in imports {
        match &import.item {
            PackageImportName::Name(name) if name == ident => {
                if let Some(res) = resolve_package_member(db, import.package.as_ref()?, ident) {
                    push_unique_resolution(&mut explicit, res);
                }
            }
            PackageImportName::Wildcard => {
                if let Some(res) = resolve_package_member(db, import.package.as_ref()?, ident) {
                    push_unique_resolution(&mut wildcard, res);
                }
            }
            PackageImportName::Name(_) => {}
        }
    }

    if explicit.is_empty() { single_resolution(wildcard) } else { single_resolution(explicit) }
}

fn resolve_package_member(
    db: &dyn HirDb,
    package: &Ident,
    ident: &Ident,
) -> Option<PathResolution> {
    let module_id = db.unit_scope().resolve_module(package).unique()?;
    resolve_package_member_by_id(db, module_id, ident, &mut FxHashSet::default())
}

fn resolve_package_member_by_id(
    db: &dyn HirDb,
    package_id: ModuleId,
    ident: &Ident,
    seen: &mut FxHashSet<(ModuleId, Ident)>,
) -> Option<PathResolution> {
    if let Some(res) = resolve_local_member_in_module(db, package_id, ident) {
        return Some(res);
    }

    if !seen.insert((package_id, ident.clone())) {
        return None;
    }

    let module = db.module(package_id);
    let mut explicit = Vec::new();
    let mut wildcard = Vec::new();

    for (_, export) in module.package_exports.iter() {
        match &export.item {
            PackageExportName::Name(name) if name == ident => {
                if let Some(package) = export.package.as_ref()
                    && let Some(target) = db.unit_scope().resolve_module(package).unique()
                    && let Some(res) = resolve_package_member_by_id(db, target, ident, seen)
                {
                    push_unique_resolution(&mut explicit, res);
                }
            }
            PackageExportName::Wildcard => {
                if let Some(package) = export.package.as_ref()
                    && let Some(target) = db.unit_scope().resolve_module(package).unique()
                    && let Some(res) = resolve_package_member_by_id(db, target, ident, seen)
                {
                    push_unique_resolution(&mut wildcard, res);
                }
            }
            PackageExportName::AllImports => {
                for (_, import) in module.package_imports.iter() {
                    match &import.item {
                        PackageImportName::Name(name) if name == ident => {
                            if let Some(package) = import.package.as_ref()
                                && let Some(target) =
                                    db.unit_scope().resolve_module(package).unique()
                                && let Some(res) =
                                    resolve_package_member_by_id(db, target, ident, seen)
                            {
                                push_unique_resolution(&mut explicit, res);
                            }
                        }
                        PackageImportName::Wildcard => {
                            if let Some(package) = import.package.as_ref()
                                && let Some(target) =
                                    db.unit_scope().resolve_module(package).unique()
                                && let Some(res) =
                                    resolve_package_member_by_id(db, target, ident, seen)
                            {
                                push_unique_resolution(&mut wildcard, res);
                            }
                        }
                        PackageImportName::Name(_) => {}
                    }
                }
            }
            PackageExportName::Name(_) => {}
        }
    }

    seen.remove(&(package_id, ident.clone()));
    if explicit.is_empty() { single_resolution(wildcard) } else { single_resolution(explicit) }
}

fn resolve_local_member_in_module(
    db: &dyn HirDb,
    module_id: ModuleId,
    ident: &Ident,
) -> Option<PathResolution> {
    db.module_scope(module_id)
        .get(ident)
        .map(|entry| PathResolution::from(InModule::new(module_id, entry)))
}

fn push_unique_resolution(resolutions: &mut Vec<PathResolution>, res: PathResolution) {
    if !resolutions.contains(&res) {
        resolutions.push(res);
    }
}

fn single_resolution(mut resolutions: Vec<PathResolution>) -> Option<PathResolution> {
    if resolutions.len() == 1 { resolutions.pop() } else { None }
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
    db.unit_scope().resolve_module(module_name).unique()
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

fn eval_const_i128(db: &dyn HirDb, container: ContainerId, expr_id: ExprId) -> Option<i128> {
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
        ContainerId::HirFileId(file_id) => Some(db.hir_file(file_id).get(expr.value).clone()),
        ContainerId::ModuleId(module_id) => Some(db.module(module_id).get(expr.value).clone()),
        ContainerId::GenerateBlockId(generate_block_id) => {
            Some(db.generate_block(generate_block_id).get(expr.value).clone())
        }
        ContainerId::BlockId(block_id) => Some(db.block(block_id).get(expr.value).clone()),
        ContainerId::SubroutineId(subroutine_id) => {
            Some(db.subroutine(subroutine_id).get(expr.value).clone())
        }
    }
}

fn decl_of(
    db: &dyn HirDb,
    decl: InContainer<DeclId>,
) -> Option<crate::hir_def::expr::declarator::Declarator> {
    match decl.cont_id {
        ContainerId::HirFileId(file_id) => Some(db.hir_file(file_id).get(decl.value).clone()),
        ContainerId::ModuleId(module_id) => Some(db.module(module_id).get(decl.value).clone()),
        ContainerId::GenerateBlockId(generate_block_id) => {
            Some(db.generate_block(generate_block_id).get(decl.value).clone())
        }
        ContainerId::BlockId(block_id) => Some(db.block(block_id).get(decl.value).clone()),
        ContainerId::SubroutineId(subroutine_id) => {
            Some(db.subroutine(subroutine_id).get(decl.value).clone())
        }
    }
}

fn declaration_of(
    db: &dyn HirDb,
    decl: InContainer<crate::hir_def::declaration::DeclarationId>,
) -> Option<Declaration> {
    match decl.cont_id {
        ContainerId::HirFileId(file_id) => Some(db.hir_file(file_id).get(decl.value).clone()),
        ContainerId::ModuleId(module_id) => Some(db.module(module_id).get(decl.value).clone()),
        ContainerId::GenerateBlockId(generate_block_id) => {
            Some(db.generate_block(generate_block_id).get(decl.value).clone())
        }
        ContainerId::BlockId(block_id) => Some(db.block(block_id).get(decl.value).clone()),
        ContainerId::SubroutineId(subroutine_id) => {
            Some(db.subroutine(subroutine_id).get(decl.value).clone())
        }
    }
}

fn typedef_of(
    db: &dyn HirDb,
    typedef: InContainer<TypedefId>,
) -> Option<crate::hir_def::typedef::Typedef> {
    match typedef.cont_id {
        ContainerId::HirFileId(file_id) => Some(db.hir_file(file_id).get(typedef.value).clone()),
        ContainerId::ModuleId(module_id) => Some(db.module(module_id).get(typedef.value).clone()),
        ContainerId::GenerateBlockId(generate_block_id) => {
            Some(db.generate_block(generate_block_id).get(typedef.value).clone())
        }
        ContainerId::BlockId(block_id) => Some(db.block(block_id).get(typedef.value).clone()),
        ContainerId::SubroutineId(subroutine_id) => {
            Some(db.subroutine(subroutine_id).get(typedef.value).clone())
        }
    }
}

fn struct_of(
    db: &dyn HirDb,
    struct_id: InContainer<StructId>,
) -> Option<crate::hir_def::aggregate::StructDef> {
    match struct_id.cont_id {
        ContainerId::HirFileId(file_id) => Some(db.hir_file(file_id).get(struct_id.value).clone()),
        ContainerId::ModuleId(module_id) => Some(db.module(module_id).get(struct_id.value).clone()),
        ContainerId::GenerateBlockId(generate_block_id) => {
            Some(db.generate_block(generate_block_id).get(struct_id.value).clone())
        }
        ContainerId::BlockId(block_id) => Some(db.block(block_id).get(struct_id.value).clone()),
        ContainerId::SubroutineId(subroutine_id) => {
            Some(db.subroutine(subroutine_id).get(struct_id.value).clone())
        }
    }
}

fn stmt_of(
    db: &dyn HirDb,
    stmt: InContainer<crate::hir_def::stmt::StmtId>,
) -> Option<crate::hir_def::stmt::Stmt> {
    match stmt.cont_id {
        ContainerId::HirFileId(file_id) => Some(db.hir_file(file_id).get(stmt.value).clone()),
        ContainerId::ModuleId(module_id) => Some(db.module(module_id).get(stmt.value).clone()),
        ContainerId::GenerateBlockId(generate_block_id) => {
            Some(db.generate_block(generate_block_id).get(stmt.value).clone())
        }
        ContainerId::BlockId(block_id) => Some(db.block(block_id).get(stmt.value).clone()),
        ContainerId::SubroutineId(subroutine_id) => {
            Some(db.subroutine(subroutine_id).get(stmt.value).clone())
        }
    }
}
