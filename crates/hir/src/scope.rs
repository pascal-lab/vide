use rustc_hash::FxHashMap;
use triomphe::Arc;
use utils::{
    define_enum_deriving_from,
    get::{Get, GetRef},
};

use crate::{
    container::InFile,
    db::HirDb,
    file::HirFileId,
    hir_def::{
        Ident,
        block::{BlockId, BlockInfo},
        expr::declarator::{DeclId, DeclaratorParent},
        file::{config::ConfigDeclId, udp::UdpDeclId},
        module::{
            ModuleId,
            generate::GenerateBlockId,
            instantiation::InstanceId,
            port::{NonAnsiPortId, Ports},
        },
        opaque::OpaqueItemId,
        stmt::{StmtId, StmtKind},
        subroutine::{SubroutineId, SubroutineLoc, SubroutinePortId, SubroutineSrc},
        typedef::TypedefId,
    },
};

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum UnitEntry {
        ModuleId,
        FiledConfigDeclId,
        FiledUdpDeclId,
        FiledDeclId,
        FiledTypedefId,
        FiledOpaqueItemId,
    }
}

pub type FiledDeclId = InFile<DeclId>;
pub type FiledConfigDeclId = InFile<ConfigDeclId>;
pub type FiledUdpDeclId = InFile<UdpDeclId>;
pub type FiledTypedefId = InFile<TypedefId>;
pub type FiledOpaqueItemId = InFile<OpaqueItemId>;

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ModuleEntry {
        DeclId,
        TypedefId,
        GenerateBlockId,
        NonAnsiPortEntry,
        AnsiPortEntry,
        InstanceId,
        StmtId,
        BlockId,
        SubroutineId,
        OpaqueItemId,
    }
}

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    pub enum GenerateBlockEntry {
        DeclId,
        TypedefId,
        GenerateBlockId,
        StmtId,
        BlockId,
        SubroutineId,
        OpaqueItemId,
    }
}

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct NonAnsiPortEntry {
    // explicit label for port
    pub label: Option<NonAnsiPortId>,
    pub port_decl: Option<DeclId>,
    pub data_decl: Option<DeclId>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct AnsiPortEntry(pub DeclId);

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    pub enum BlockEntry {
        StmtId,
        DeclId,
        TypedefId,
        BlockId,
        OpaqueItemId,
    }
}

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    pub enum SubroutineEntry {
        StmtId,
        DeclId,
        TypedefId,
        BlockId,
        SubroutinePortId,
        OpaqueItemId,
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Scope<Entry> {
    entries: FxHashMap<Ident, Entry>,
}

impl<Entry> Default for Scope<Entry> {
    fn default() -> Self {
        Scope { entries: FxHashMap::default() }
    }
}

impl<Entry: Copy> Scope<Entry> {
    pub(crate) fn insert(&mut self, ident: &Ident, entry: Entry) -> Option<Entry> {
        self.entries.insert(ident.clone(), entry)
    }

    pub(crate) fn insert_opt(&mut self, ident: &Option<Ident>, entry: Entry) -> Option<Entry> {
        self.insert(ident.as_ref()?, entry)
    }

    pub fn get(&self, ident: &Ident) -> Option<Entry> {
        self.entries.get(ident).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Ident, Entry)> + '_ {
        self.entries.iter().map(|(k, v)| (k, *v))
    }

    pub(crate) fn get_mut(&mut self, ident: &Ident) -> Option<&mut Entry> {
        self.entries.get_mut(ident)
    }
}

pub type UnitScope = Scope<UnitEntry>;
pub type ModuleScope = Scope<ModuleEntry>;
pub type GenerateBlockScope = Scope<GenerateBlockEntry>;
pub type BlockScope = Scope<BlockEntry>;
pub type SubroutineScope = Scope<SubroutineEntry>;

// TODO: diagnostics

impl UnitScope {
    pub fn unit_scope_query(db: &dyn HirDb) -> Arc<UnitScope> {
        let mut scope = Scope::default();

        for file_id in db.files().iter() {
            let file_id = HirFileId(*file_id);
            let file_scope = db.file_scope(file_id);
            scope.entries.extend(file_scope.entries.clone());
        }

        Arc::new(scope)
    }

    pub(super) fn file_scope_query(db: &dyn HirDb, file_id: HirFileId) -> Arc<UnitScope> {
        let mut scope = Scope::default();
        let hir_file = db.hir_file(file_id);

        for (module_id, module_info) in hir_file.modules.iter() {
            scope.insert_opt(&module_info.name, InFile::new(file_id, module_id).into());
        }

        for (decl_id, decl) in hir_file.decls.iter() {
            scope.insert_opt(&decl.name, InFile::new(file_id, decl_id).into());
        }

        for (config_decl_id, config_decl) in hir_file.config_decls.iter() {
            scope.insert_opt(&config_decl.name, InFile::new(file_id, config_decl_id).into());
        }

        for (udp_decl_id, udp_decl) in hir_file.udp_decls.iter() {
            scope.insert_opt(&udp_decl.name, InFile::new(file_id, udp_decl_id).into());
        }

        for (typedef_id, typedef) in hir_file.typedefs.iter() {
            scope.insert_opt(&typedef.name, InFile::new(file_id, typedef_id).into());
        }

        for (opaque_id, opaque) in hir_file.opaque_items.iter() {
            scope.insert_opt(&opaque.name, InFile::new(file_id, opaque_id).into());
        }

        Arc::new(scope)
    }
}

impl ModuleScope {
    pub fn module_scope_query(db: &dyn HirDb, module_id: ModuleId) -> Arc<ModuleScope> {
        let mut scope = Scope::default();
        let (module, module_src_map) = db.module_with_source_map(module_id);
        let file_id = HirFileId(module_id.file_id());

        // handle labels of non-ansi ports
        if let Ports::NonAnsi { ports, .. } = &module.ports {
            for (port_id, port) in ports.iter() {
                let entry = NonAnsiPortEntry { label: Some(port_id), ..Default::default() }.into();
                scope.insert_opt(&port.label, entry);
            }
        }

        for (local_subroutine_id, subroutine) in module.subroutines.iter() {
            let src: SubroutineSrc = module_src_map.get(local_subroutine_id);
            let subroutine_id = db.intern_subroutine(SubroutineLoc {
                cont_id: module_id.into(),
                src: InFile::new(file_id, src),
            });
            scope.insert_opt(&subroutine.name, subroutine_id.into());
        }

        // handle other members
        for (decl_id, decl) in module.decls.iter() {
            let Some(name) = &decl.name else {
                continue;
            };

            let is_port_decl = matches!(decl.parent, DeclaratorParent::PortDeclId(_));

            if let Some(ModuleEntry::NonAnsiPortEntry(entry)) = scope.get_mut(name) {
                if is_port_decl {
                    entry.port_decl = Some(decl_id);
                } else {
                    entry.data_decl = Some(decl_id);
                }
                continue;
            }

            let entry = if is_port_decl {
                match module.ports {
                    Ports::NonAnsi { .. } => {
                        NonAnsiPortEntry { port_decl: Some(decl_id), ..Default::default() }.into()
                    }
                    Ports::Ansi(_) => AnsiPortEntry(decl_id).into(),
                }
            } else {
                decl_id.into()
            };

            scope.insert(name, entry);
        }

        for (typedef_id, typedef) in module.typedefs.iter() {
            scope.insert_opt(&typedef.name, typedef_id.into());
        }

        for (opaque_id, opaque) in module.opaque_items.iter() {
            scope.insert_opt(&opaque.name, opaque_id.into());
        }

        for (instance_id, instance) in module.instances.iter() {
            scope.insert_opt(&instance.name, instance_id.into());
        }

        for item in &module_src_map.items {
            if let crate::hir_def::module::ModuleItem::GenerateRegionId(generate_region_id) = item {
                let generate_region = module.get(*generate_region_id);
                for item in &generate_region.items {
                    if let crate::hir_def::module::generate::GenerateItem::GenerateBlockId(
                        generate_block_id,
                    ) = *item
                    {
                        let generate_block = db.generate_block(generate_block_id);
                        scope.insert_opt(&generate_block.name, generate_block_id.into());
                    }
                }
            }
        }

        for (stmt_id, stmt) in module.stmts.iter() {
            scope.insert_opt(&stmt.label, stmt_id.into());

            if let StmtKind::Block(BlockInfo { name, block_id }) = &stmt.kind {
                scope.insert_opt(name, (*block_id).into());
            }
        }

        Arc::new(scope)
    }
}

impl GenerateBlockScope {
    pub fn generate_block_scope_query(
        db: &dyn HirDb,
        generate_block_id: GenerateBlockId,
    ) -> Arc<Self> {
        let mut scope = Scope::default();
        let (generate_block, source_map) = db.generate_block_with_source_map(generate_block_id);
        let file_id = HirFileId(generate_block_id.file_id(db));

        scope.insert_opt(&generate_block.name, generate_block_id.into());

        for (local_subroutine_id, subroutine) in generate_block.subroutines.iter() {
            let src: SubroutineSrc = source_map.get(local_subroutine_id);
            let subroutine_id = db.intern_subroutine(SubroutineLoc {
                cont_id: generate_block_id.into(),
                src: InFile::new(file_id, src),
            });
            scope.insert_opt(&subroutine.name, subroutine_id.into());
        }

        for (decl_id, decl) in generate_block.decls.iter() {
            scope.insert_opt(&decl.name, decl_id.into());
        }

        for (typedef_id, typedef) in generate_block.typedefs.iter() {
            scope.insert_opt(&typedef.name, typedef_id.into());
        }

        for item in &generate_block.items {
            if let crate::hir_def::module::generate::GenerateBlockItem::GenerateBlockId(child_id) =
                *item
            {
                let child = db.generate_block(child_id);
                scope.insert_opt(&child.name, child_id.into());
            }
        }

        for (opaque_id, opaque) in generate_block.opaque_items.iter() {
            scope.insert_opt(&opaque.name, opaque_id.into());
        }

        for (stmt_id, stmt) in generate_block.stmts.iter() {
            scope.insert_opt(&stmt.label, stmt_id.into());

            if let StmtKind::Block(BlockInfo { name, block_id }) = &stmt.kind {
                scope.insert_opt(name, (*block_id).into());
            }
        }

        Arc::new(scope)
    }
}

impl BlockScope {
    pub fn block_scope_query(db: &dyn HirDb, block_id: BlockId) -> Arc<BlockScope> {
        let mut scope = Scope::default();
        let block = db.block(block_id);

        for (decl_id, decl) in block.decls.iter() {
            scope.insert_opt(&decl.name, decl_id.into());
        }

        for (typedef_id, typedef) in block.typedefs.iter() {
            scope.insert_opt(&typedef.name, typedef_id.into());
        }

        for (opaque_id, opaque) in block.opaque_items.iter() {
            scope.insert_opt(&opaque.name, opaque_id.into());
        }

        for (stmt_id, stmt) in block.stmts.iter() {
            scope.insert_opt(&stmt.label, stmt_id.into());

            if let StmtKind::Block(BlockInfo { name, block_id }) = &stmt.kind {
                scope.insert_opt(name, (*block_id).into());
            }
        }

        Arc::new(scope)
    }
}

impl SubroutineScope {
    pub fn subroutine_scope_query(db: &dyn HirDb, subroutine_id: SubroutineId) -> Arc<Self> {
        let mut scope = Scope::default();
        let subroutine = db.subroutine(subroutine_id);

        for (port_idx, port) in subroutine.ports.iter().enumerate() {
            let port_id = SubroutinePortId(port_idx as u32);
            scope.insert_opt(&port.name, port_id.into());
        }

        for (decl_id, decl) in subroutine.decls.iter() {
            scope.insert_opt(&decl.name, decl_id.into());
        }

        for (typedef_id, typedef) in subroutine.typedefs.iter() {
            scope.insert_opt(&typedef.name, typedef_id.into());
        }

        for (opaque_id, opaque) in subroutine.opaque_items.iter() {
            scope.insert_opt(&opaque.name, opaque_id.into());
        }

        for (stmt_id, stmt) in subroutine.stmts.iter() {
            scope.insert_opt(&stmt.label, stmt_id.into());

            if let StmtKind::Block(BlockInfo { name, block_id }) = &stmt.kind {
                scope.insert_opt(name, (*block_id).into());
            }
        }

        Arc::new(scope)
    }
}
