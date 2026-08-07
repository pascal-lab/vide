use preproc_expand::file::HirFileId;
use smallvec::SmallVec;
use smol_str::SmolStr;
use triomphe::Arc;
use utils::{
    define_enum_deriving_from,
    get::{Get, GetRef},
};

use crate::{
    aggregate::{StructDef, StructId, StructSrc},
    ast_id_map::SourceAstId,
    body::{Body, BodySourceMap},
    checker::CheckerId,
    covergroup::CovergroupId,
    db::HirDefDb,
    declaration::{Declaration, DeclarationId, DeclarationSrc},
    expr::{
        Expr, ExprId, ExprSrc,
        declarator::{DeclId, Declarator, DeclaratorSrc},
        timing_control::{EventExpr, EventExprId, EventExprSrc},
    },
    module::{
        Module, ModuleId, ModuleKind, ModuleSourceMap,
        clocking::ClockingBlockId,
        generate::{GenerateBlock, GenerateBlockId, GenerateBlockSourceMap},
    },
    owner::{OwnerId, OwnerKind},
    region_tree::RegionTree,
    stmt::{Stmt, StmtId, StmtSrc},
    subroutine::LocalSubroutineId,
    symbol::ScopeKind,
    typedef::{Typedef, TypedefId, TypedefSrc},
};

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
    pub enum ScopeId {
        File(HirFileId),
        Module(ModuleId),
        GenerateBlock(GenerateBlockId),
        Subroutine(SubroutineScope),
        Owner(OwnerId),
        ClockingBlock(InModule<ClockingBlockId>),
        Checker(InFileOrModule<CheckerId>),
        Covergroup(InFileOrModule<CovergroupId>),
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct InScope<T> {
    pub value: T,
    pub scope_id: ScopeId,
}

impl<T> InScope<T> {
    pub fn new(scope_id: ScopeId, value: T) -> Self {
        Self { value, scope_id }
    }

    pub fn with_value<U>(&self, value: U) -> InScope<U> {
        InScope::new(self.scope_id.clone(), value)
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> InScope<U> {
        InScope::new(self.scope_id, f(self.value))
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct InFileOrModule<T> {
    pub value: T,
    pub cont_id: FileOrModule,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub enum FileOrModule {
    File(HirFileId),
    Module(ModuleId),
}

impl<T> InFileOrModule<T> {
    pub fn new(cont_id: FileOrModule, value: T) -> Self {
        Self { value, cont_id }
    }

    pub fn parent_scope(&self) -> ScopeId {
        self.cont_id.into()
    }
}

impl FileOrModule {
    pub fn file_id(self) -> HirFileId {
        match self {
            FileOrModule::File(file_id) => file_id,
            FileOrModule::Module(module_id) => module_id.file_id,
        }
    }
}

impl From<FileOrModule> for ScopeId {
    fn from(cont_id: FileOrModule) -> Self {
        match cont_id {
            FileOrModule::File(file_id) => ScopeId::File(file_id),
            FileOrModule::Module(module_id) => ScopeId::Module(module_id),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct SubroutineScope {
    pub cont_id: SubroutineParent,
    pub value: LocalSubroutineId,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub enum SubroutineParent {
    File(HirFileId),
    Module(ModuleId),
    GenerateBlock(GenerateBlockId),
}

impl SubroutineScope {
    pub fn new(cont_id: SubroutineParent, value: LocalSubroutineId) -> Self {
        Self { cont_id, value }
    }

    pub fn parent_scope(self) -> ScopeId {
        self.cont_id.into()
    }

    pub fn file_id(self, db: &dyn HirDefDb) -> HirFileId {
        match self.cont_id {
            SubroutineParent::File(file_id) => file_id,
            SubroutineParent::Module(module_id) => module_id.file_id,
            SubroutineParent::GenerateBlock(generate_block_id) => generate_block_id.file_id(db),
        }
    }
}

impl From<SubroutineParent> for ScopeId {
    fn from(cont_id: SubroutineParent) -> Self {
        match cont_id {
            SubroutineParent::File(file_id) => ScopeId::File(file_id),
            SubroutineParent::Module(module_id) => ScopeId::Module(module_id),
            SubroutineParent::GenerateBlock(block) => ScopeId::GenerateBlock(block),
        }
    }
}

/// An owner-local arena index. `OwnerId` selects the only store; `value` is
/// meaningful only inside that store.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct InContainer<T> {
    pub value: T,
    pub cont_id: OwnerId,
}

impl<T> InContainer<T> {
    pub fn new(cont_id: OwnerId, value: T) -> InContainer<T> {
        InContainer { value, cont_id }
    }

    pub fn with_value<U>(&self, value: U) -> InContainer<U> {
        InContainer::<U>::new(self.cont_id, value)
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> InContainer<U> {
        InContainer::new(self.cont_id, f(self.value))
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct InSubroutine<T> {
    pub value: T,
    pub subroutine: SubroutineScope,
}

impl<T> InSubroutine<T> {
    pub fn new(subroutine: SubroutineScope, value: T) -> Self {
        Self { value, subroutine }
    }

    pub fn with_value<U>(self, value: U) -> InSubroutine<U> {
        InSubroutine { value, subroutine: self.subroutine }
    }
}

macro_rules! define_container_id {
    ($($name:ident[$id:ident : $ty:ty]),* $(,)?) => {
        $(
            #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
            pub struct $name<T> {
                pub value: T,
                pub $id: $ty,
            }

            impl<T> $name<T> {
                pub fn new($id: $ty, value: T) -> Self {
                    Self { value, $id }
                }

                pub fn with_value<U>(self, value: U) -> $name<U> {
                    $name::<U>::new(self.$id, value)
                }

                pub fn map<U>(self, f: impl FnOnce(T) -> U) -> $name<U> {
                    $name::new(self.$id, f(self.value))
                }
            }

            // Typed wrappers retain their concrete container projection; they
            // are not implicitly converted into owner-local arena references.
        )*
    };
}

define_container_id! {
    InFile[file_id: HirFileId],
    InModule[module_id: ModuleId],
    InGenerateBlock[generate_block_id: GenerateBlockId],
}
impl<T: Copy> Copy for InFile<T> {}
impl<T: Copy> Copy for InModule<T> {}

impl ScopeId {
    /// Canonical owner identity for every scope wrapper.
    pub fn owner(&self, db: &dyn HirDefDb) -> OwnerId {
        match self {
            ScopeId::File(file_id) => db
                .owner_table(*file_id)
                .file_owner()
                .expect("file scope must have a canonical owner"),
            ScopeId::Module(module_id) => module_id.owner(db).expect("module scope owner"),
            ScopeId::GenerateBlock(generate) => {
                generate.clone().owner(db).expect("generate scope owner")
            }
            ScopeId::Subroutine(subroutine) => {
                subroutine.clone().owner(db).expect("subroutine scope owner")
            }
            ScopeId::Owner(owner) => *owner,
            ScopeId::ClockingBlock(clocking) => {
                let source = db
                    .module_with_source_map(clocking.module_id)
                    .source(clocking.value)
                    .expect("clocking scope source");
                OwnerId::new(db, clocking.module_id.file_id, source, OwnerKind::ClockingBlock)
            }
            ScopeId::Checker(checker) => {
                let (file_id, source) = source_of_checker(db, *checker);
                OwnerId::new(db, file_id, source, OwnerKind::Checker)
            }
            ScopeId::Covergroup(covergroup) => {
                let (file_id, source) = source_of_covergroup(db, *covergroup);
                OwnerId::new(db, file_id, source, OwnerKind::Covergroup)
            }
        }
    }

    pub(crate) fn from_owner(db: &dyn HirDefDb, owner: OwnerId) -> Option<Self> {
        let parent = owner.parent(db)?;
        match owner.kind(db) {
            OwnerKind::Checker => {
                let cont_id = match parent.kind(db) {
                    OwnerKind::File => FileOrModule::File(parent.file(db)),
                    OwnerKind::Module => FileOrModule::Module(ModuleId::from_owner(db, parent)?),
                    _ => return None,
                };
                let value = match cont_id {
                    FileOrModule::File(file_id) => db
                        .hir_file_with_source_map(file_id)
                        .source_map()
                        .checker_srcs
                        .iter()
                        .find(|(_, source)| *source == owner.ast_id(db))
                        .map(|(id, _)| id)?,
                    FileOrModule::Module(module_id) => db
                        .module_with_source_map(module_id)
                        .source_map()
                        .checker_srcs
                        .iter()
                        .find(|(_, source)| *source == owner.ast_id(db))
                        .map(|(id, _)| id)?,
                };
                Some(ScopeId::Checker(InFileOrModule::new(cont_id, value)))
            }
            OwnerKind::Covergroup => {
                let cont_id = match parent.kind(db) {
                    OwnerKind::File => FileOrModule::File(parent.file(db)),
                    OwnerKind::Module => FileOrModule::Module(ModuleId::from_owner(db, parent)?),
                    _ => return None,
                };
                let value = match cont_id {
                    FileOrModule::File(file_id) => db
                        .hir_file_with_source_map(file_id)
                        .source_map()
                        .covergroup_srcs
                        .iter()
                        .find(|(_, source)| *source == owner.ast_id(db))
                        .map(|(id, _)| id)?,
                    FileOrModule::Module(module_id) => db
                        .module_with_source_map(module_id)
                        .source_map()
                        .covergroup_srcs
                        .iter()
                        .find(|(_, source)| *source == owner.ast_id(db))
                        .map(|(id, _)| id)?,
                };
                Some(ScopeId::Covergroup(InFileOrModule::new(cont_id, value)))
            }
            OwnerKind::ClockingBlock => {
                let module = ModuleId::from_owner(db, parent)?;
                let value = db
                    .module_with_source_map(module)
                    .source_map()
                    .clocking_block_srcs
                    .iter()
                    .find(|(_, source)| *source == owner.ast_id(db))
                    .map(|(id, _)| id)?;
                Some(ScopeId::ClockingBlock(InModule::new(module, value)))
            }
            _ => None,
        }
    }
}

fn source_of_checker(
    db: &dyn HirDefDb,
    checker: InFileOrModule<CheckerId>,
) -> (HirFileId, SourceAstId) {
    match checker.cont_id {
        FileOrModule::File(file_id) => (
            file_id,
            db.hir_file_with_source_map(file_id).source(checker.value).expect("checker source"),
        ),
        FileOrModule::Module(module_id) => (
            module_id.file_id,
            db.module_with_source_map(module_id).source(checker.value).expect("checker source"),
        ),
    }
}

fn source_of_covergroup(
    db: &dyn HirDefDb,
    covergroup: InFileOrModule<CovergroupId>,
) -> (HirFileId, SourceAstId) {
    match covergroup.cont_id {
        FileOrModule::File(file_id) => (
            file_id,
            db.hir_file_with_source_map(file_id)
                .source(covergroup.value)
                .expect("covergroup source"),
        ),
        FileOrModule::Module(module_id) => (
            module_id.file_id,
            db.module_with_source_map(module_id)
                .source(covergroup.value)
                .expect("covergroup source"),
        ),
    }
}

impl ScopeId {
    pub fn kind(self, db: &dyn HirDefDb) -> ScopeKind {
        let owner = self.owner(db);
        match owner.kind(db) {
            OwnerKind::File => ScopeKind::File,
            OwnerKind::Module => ModuleId::from_owner(db, owner)
                .map(|module| match db.hir_file(module.file_id).get(module.value).kind {
                    ModuleKind::Module => ScopeKind::Module,
                    ModuleKind::Interface => ScopeKind::Interface,
                    ModuleKind::Program => ScopeKind::Program,
                    ModuleKind::Package => ScopeKind::Package,
                })
                .unwrap_or(ScopeKind::Module),
            OwnerKind::GenerateBlock => ScopeKind::GenerateBlock,
            OwnerKind::ProceduralBlock => ScopeKind::ProceduralBlock,
            OwnerKind::Block => ScopeKind::Block,
            OwnerKind::Subroutine => ScopeKind::Subroutine,
            OwnerKind::Checker => ScopeKind::Checker,
            OwnerKind::Covergroup => ScopeKind::Covergroup,
            OwnerKind::ClockingBlock => ScopeKind::ClockingBlock,
        }
    }

    pub fn name(self, db: &dyn HirDefDb) -> Option<SmolStr> {
        match self {
            ScopeId::File(_) => None,
            ScopeId::Module(module_id) => db.module(module_id).name.clone(),
            ScopeId::GenerateBlock(generate_block_id) => {
                db.generate_block(generate_block_id).name.clone()
            }
            ScopeId::Subroutine(subroutine) => db.subroutine(subroutine).name.clone(),
            ScopeId::Owner(owner) => {
                db.owner_table(owner.file(db)).owner(owner).map(|data| data.name.clone())
            }
            ScopeId::ClockingBlock(clocking_block) => {
                db.module(clocking_block.module_id).get(clocking_block.value).name.clone()
            }
            ScopeId::Checker(checker) => match checker.cont_id {
                FileOrModule::File(file_id) => db.hir_file(file_id).get(checker.value).name.clone(),
                FileOrModule::Module(module_id) => {
                    db.module(module_id).get(checker.value).name.clone()
                }
            },
            ScopeId::Covergroup(covergroup) => match covergroup.cont_id {
                FileOrModule::File(file_id) => {
                    db.hir_file(file_id).get(covergroup.value).name.clone()
                }
                FileOrModule::Module(module_id) => {
                    db.module(module_id).get(covergroup.value).name.clone()
                }
            },
        }
    }

    pub fn arena_owner(&self, db: &dyn HirDefDb) -> OwnerId {
        self.owner(db)
    }

    pub fn file_id(&self, db: &dyn HirDefDb) -> HirFileId {
        match self {
            ScopeId::File(file_id) => *file_id,
            ScopeId::Module(module_id) => module_id.file_id,
            ScopeId::GenerateBlock(generate_block_id) => generate_block_id.file_id(db),
            ScopeId::Subroutine(subroutine) => subroutine.clone().file_id(db),
            ScopeId::Owner(owner) => owner.file(db),
            ScopeId::ClockingBlock(clocking_block) => clocking_block.module_id.file_id,
            ScopeId::Checker(checker) => checker.cont_id.file_id(),
            ScopeId::Covergroup(covergroup) => covergroup.cont_id.file_id(),
        }
    }
}

/// Name-resolution-only scopes cannot access owner-local arena data:
///
/// ```compile_fail
/// use hir_def::{container::ScopeId, db::HirDefDb};
///
/// fn data_for_any_scope(scope: ScopeId, db: &dyn HirDefDb) {
///     let _ = scope.data(db);
/// }
/// ```
///
/// Access to the one owner-local HIR store.
impl OwnerId {
    pub fn file_id(self, db: &dyn HirDefDb) -> HirFileId {
        self.file(db)
    }

    pub fn data(self, db: &dyn HirDefDb) -> Container {
        Container(db.body_with_source_map(self).data())
    }

    pub fn source_map(self, db: &dyn HirDefDb) -> ContainerSrcMap {
        ContainerSrcMap(db.body_with_source_map(self).source_map_arc())
    }
}

impl ModuleId {
    #[inline]
    pub fn to_container(&self, db: &dyn HirDefDb) -> Arc<Module> {
        db.module(*self)
    }

    #[inline]
    pub fn to_container_src_map(&self, db: &dyn HirDefDb) -> Arc<ModuleSourceMap> {
        db.module_with_source_map(*self).source_map_arc()
    }
}

impl GenerateBlockId {
    pub fn file_id(&self, _db: &dyn HirDefDb) -> HirFileId {
        self.loc().src.file_id
    }

    #[inline]
    pub fn to_container(&self, db: &dyn HirDefDb) -> Arc<GenerateBlock> {
        db.generate_block(self.clone())
    }

    #[inline]
    pub fn to_container_src_map(&self, db: &dyn HirDefDb) -> Arc<GenerateBlockSourceMap> {
        db.generate_block_with_source_map(self.clone()).source_map_arc()
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Container(Arc<Body>);

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ContainerSrcMap(Arc<BodySourceMap>);

impl Container {
    pub fn declaration(&self, id: DeclarationId) -> &Declaration {
        &self.0.declarations[id]
    }

    pub fn typedef(&self, id: TypedefId) -> &Typedef {
        &self.0.typedefs[id]
    }

    pub fn struct_def(&self, id: StructId) -> &StructDef {
        &self.0.structs[id]
    }

    pub fn expr(&self, id: ExprId) -> &Expr {
        &self.0.exprs[id]
    }

    pub fn event_expr(&self, id: EventExprId) -> &EventExpr {
        &self.0.event_exprs[id]
    }

    pub fn declarator(&self, id: DeclId) -> &Declarator {
        &self.0.decls[id]
    }

    pub fn stmt(&self, id: StmtId) -> &Stmt {
        &self.0.stmts[id]
    }
}

impl ContainerSrcMap {
    pub fn declaration_from_source(&self, src: DeclarationSrc) -> Option<DeclarationId> {
        self.0.declaration_srcs.get(src)
    }

    pub fn source_of_declaration(&self, id: DeclarationId) -> Option<DeclarationSrc> {
        self.0.declaration_srcs.get(id)
    }

    pub fn typedef_from_source(&self, src: TypedefSrc) -> Option<TypedefId> {
        self.0.typedef_srcs.get(src)
    }

    pub fn source_of_typedef(&self, id: TypedefId) -> Option<TypedefSrc> {
        self.0.typedef_srcs.get(id)
    }

    pub fn struct_from_source(&self, src: StructSrc) -> Option<StructId> {
        self.0.struct_srcs.get(src)
    }

    pub fn source_of_struct(&self, id: StructId) -> Option<StructSrc> {
        self.0.struct_srcs.get(id)
    }

    pub fn expr_from_source(&self, src: ExprSrc) -> Option<ExprId> {
        self.0.expr_srcs.get(src)
    }

    pub fn source_of_expr(&self, id: ExprId) -> Option<ExprSrc> {
        self.0.expr_srcs.get(id)
    }

    pub fn event_expr_from_source(&self, src: EventExprSrc) -> Option<EventExprId> {
        self.0.event_expr_srcs.get(src)
    }

    pub fn source_of_event_expr(&self, id: EventExprId) -> Option<EventExprSrc> {
        self.0.event_expr_srcs.get(id)
    }

    pub fn declarator_from_source(&self, src: DeclaratorSrc) -> Option<DeclId> {
        self.0.decl_srcs.get(src)
    }

    pub fn source_of_declarator(&self, id: DeclId) -> Option<DeclaratorSrc> {
        self.0.decl_srcs.get(id)
    }

    pub fn stmt_from_source(&self, src: StmtSrc) -> Option<StmtId> {
        self.0.stmt_srcs.get(src)
    }

    pub fn source_of_stmt(&self, id: StmtId) -> Option<StmtSrc> {
        self.0.stmt_srcs.get(id)
    }
}

/// An explicit lexical scope chain, ordered from the innermost scope outward.
///
/// Keeping the order in a value object prevents callers from rebuilding the
/// parent walk independently and accidentally changing shadowing precedence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeChain {
    ids: SmallVec<[ScopeId; 4]>,
}

impl ScopeChain {
    pub fn from_inner(db: &dyn HirDefDb, scope_id: ScopeId) -> Self {
        Self { ids: ScopeParent::start_from(db, scope_id).collect() }
    }

    pub fn ids(&self) -> &[ScopeId] {
        &self.ids
    }

    pub fn iter(&self) -> impl Iterator<Item = &ScopeId> {
        self.ids.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

/// Parents of a scope.
pub struct ScopeParent<'db> {
    db: &'db dyn HirDefDb,
    cont_id: Option<ScopeId>,
}

impl<'db> ScopeParent<'db> {
    pub fn start_from(db: &'db dyn HirDefDb, cont_id: ScopeId) -> ScopeParent<'db> {
        ScopeParent { db, cont_id: Some(cont_id) }
    }
}

impl Iterator for ScopeParent<'_> {
    type Item = ScopeId;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.cont_id.clone();
        self.cont_id = match self.cont_id.clone()? {
            ScopeId::File(_) => None,
            ScopeId::Module(module_id) => Some(module_id.file_id.into()),
            ScopeId::GenerateBlock(generate_block_id) => {
                Some(generate_block_id.loc().cont_id.clone().into())
            }
            ScopeId::Subroutine(subroutine) => Some(subroutine.parent_scope()),
            ScopeId::Owner(owner) => owner.parent(self.db).map(ScopeId::Owner),
            ScopeId::ClockingBlock(clocking_block) => Some(clocking_block.module_id.into()),
            ScopeId::Checker(checker) => Some(checker.parent_scope()),
            ScopeId::Covergroup(covergroup) => Some(covergroup.parent_scope()),
        };
        next
    }
}
