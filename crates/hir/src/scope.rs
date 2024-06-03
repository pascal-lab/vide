use crate::{
    container::InFile,
    db::HirDb,
    file::HirFileId,
    hir_def::{
        block::{BlockId, BlockItemDecl},
        data::{DataDecl, SubDecl},
        module::{
            module_item::{HierarchicalInst, ModuleInst, ModuleItem},
            port::PortDecl,
            Module,
        },
        pack_or_gen_item::PackOrGenItemDecl,
        stmt::{Stmt, StmtItem},
        FileItem, Ident, ModuleId,
    },
};
use la_arena::Idx;
use rustc_hash::FxHashMap;
use std::collections::hash_map;
use triomphe::Arc;
use utils::impl_from;

trait IntoScope {
    type Entry: Copy;
    type Id: Copy;

    fn id(&self) -> Self::Id;

    fn entries(&self) -> &FxHashMap<Ident, Self::Entry>;

    fn entries_mut(&mut self) -> &mut FxHashMap<Ident, Self::Entry>;

    fn insert_entry(&mut self, ident: Ident, entry: Self::Entry) {
        match self.entries_mut().entry(ident) {
            hash_map::Entry::Occupied(_) => todo!("diagnostics"),
            hash_map::Entry::Vacant(e) => e.insert(entry),
        };
    }

    fn get_entry(&self, ident: &Ident) -> Option<Self::Entry> {
        self.entries().get(ident).copied()
    }
}

macro_rules! impl_scope {
    ($($scope:ident[$entry:ident; $id:ident: $id_type:ident]),*) => {
        $(
            impl IntoScope for $scope {
                type Entry = $entry;
                type Id = $id_type;

                fn id(&self) -> Self::Id {
                    self.$id
                }

                fn entries(&self) -> &FxHashMap<Ident, Self::Entry> {
                    &self.entries
                }

                fn entries_mut(&mut self) -> &mut FxHashMap<Ident, Self::Entry> {
                    &mut self.entries
                }
            }
        )*

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum Scope {
            $(
                $scope(Arc<$scope>),
            )*
        }

        impl_from!($(Arc<$scope> as $scope),* for Scope);

        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        pub enum ScopeEntry {
            $(
                $entry($entry),
            )*
        }

        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        pub enum ScopeId {
            $(
                $id_type($id_type),
            )*
        }

        impl Scope {
            pub fn get_entry(&self, ident: &Ident) -> Option<ScopeEntry> {
                match self {
                    $(
                        Scope::$scope(scope) => scope.get_entry(ident).map(ScopeEntry::$entry),
                    )*
                }
            }

            pub fn id(&self) -> ScopeId {
                match self {
                    $(
                        Scope::$scope(scope) => ScopeId::$id_type(scope.id()),
                    )*
                }
            }
        }
    };
}

impl_scope!(
    UnitScope[UnitScopeEntry; unit_id: UnitId],
    ModuleScope[ModuleScopeEntry; module_id: ModuleId],
    BlockScope[BlockScopeEntry; block_id: BlockId]
);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct UnitId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitScope {
    pub unit_id: UnitId,
    pub entries: FxHashMap<Ident, UnitScopeEntry>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum UnitScopeEntry {
    Module(ModuleId),
    //Data(Idx<SubDecl>),
    //TF()
}

impl UnitScope {
    pub fn unit_scope_query(db: &dyn HirDb) -> Arc<UnitScope> {
        let mut scope = UnitScope { unit_id: UnitId, entries: FxHashMap::default() };

        db.files().iter().map(|file_id| HirFileId(*file_id)).for_each(|file_id| {
            scope.collect_hir_file(db, &file_id);
        });

        Arc::new(scope)
    }

    fn collect_hir_file(&mut self, db: &dyn HirDb, file_id: &HirFileId) {
        let hir_file = db.hir_file(*file_id);
        let hir_file = hir_file.as_ref();

        for item in &hir_file.items {
            match item {
                FileItem::Module(module_id) => {
                    let module = &hir_file.data[*module_id];
                    self.insert_entry(
                        module.ident.clone(),
                        UnitScopeEntry::Module(InFile::new(*file_id, *module_id)),
                    );
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleScope {
    pub module_id: ModuleId,
    pub entries: FxHashMap<Ident, ModuleScopeEntry>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ModuleScopeEntry {
    SubDecl(Idx<SubDecl>),
    PortDecl { port: Idx<SubDecl>, data: Option<Idx<SubDecl>> },
    HierarchyInst(Idx<HierarchicalInst>),
    Block(BlockId),
    Stmt(Idx<Stmt>),
    // TODO: TF()
}

impl ModuleScope {
    pub fn module_scope_query(db: &dyn HirDb, module_id: ModuleId) -> Arc<ModuleScope> {
        let module = db.module(module_id);
        let module = module.as_ref();
        let mut scope = ModuleScope {
            module_id,
            // block_scopes: ArenaMap::default(),
            entries: FxHashMap::default(),
        };

        scope.collect_param_port_list(module);
        // TODO: do we need to collect port decls?
        scope.collect_port_decls(module);
        scope.collect_module_items(module);

        Arc::new(scope)
    }

    fn collect_data_decl(&mut self, module: &Module, idx: Idx<DataDecl>) {
        let data_decl = &module[idx];
        let sub_decls = match data_decl {
            DataDecl::NetDecl(net_decl) => &net_decl.sub_decls,
            DataDecl::ParamDecl(param_decl) => &param_decl.sub_decls,
            DataDecl::VarDecl(var_decl) => &var_decl.sub_decls,
        }
        .clone();
        for sub_decl_idx in sub_decls {
            let ident = module[sub_decl_idx].ident.clone();

            use hash_map::Entry;
            match self.entries.entry(ident) {
                Entry::Occupied(mut e) => match e.get_mut() {
                    ModuleScopeEntry::PortDecl { data: data @ None, .. } => {
                        *data = Some(sub_decl_idx);
                    }
                    ModuleScopeEntry::PortDecl { .. } => todo!("diagnostics"),
                    _ => todo!("diagnostics"),
                },
                Entry::Vacant(e) => {
                    e.insert(ModuleScopeEntry::SubDecl(sub_decl_idx));
                }
            }
        }
    }

    fn collect_param_port_list(&mut self, module: &Module) {
        if let Some(param_port_list) = &module.param_port_list {
            let param_port_list = param_port_list.clone();
            for idx in param_port_list {
                self.collect_data_decl(module, idx)
            }
        }
    }

    fn collect_hierarchy_inst(&mut self, module: &Module, idx: Idx<HierarchicalInst>) {
        let inst = &module[idx];
        let ident = inst.ident.clone();
        let entry = ModuleScopeEntry::HierarchyInst(idx);
        self.insert_entry(ident, entry);
    }

    fn collect_module_inst(&mut self, module: &Module, idx: Idx<ModuleInst>) {
        let inst = &module[idx];
        for hinst_idx in inst.hierarchical_insts.clone() {
            self.collect_hierarchy_inst(module, hinst_idx);
        }
    }

    fn collect_port_decls(&mut self, module: &Module) {
        module.data.port_decls.iter().for_each(|(_, port_decl)| {
            match port_decl {
                PortDecl::IOPortDef(io_decl) => {
                    for sub_decl_idx in io_decl.sub_decls.clone() {
                        let ident = module[sub_decl_idx].ident.clone();
                        let entry = ModuleScopeEntry::PortDecl { port: sub_decl_idx, data: None };
                        self.insert_entry(ident, entry);
                    }
                }
            }
        });
    }

    fn collect_module_items(&mut self, module: &Module) {
        module.module_items.iter().for_each(|(_, item)| {
            match item {
                ModuleItem::PackOrGenItemDecl(decl) => match decl {
                    PackOrGenItemDecl::DataDecl(idx) => self.collect_data_decl(module, *idx),
                },
                ModuleItem::ProcessConstruct(pc) => {
                    if let Some(stmt) = pc.stmt {
                        if let Some(ident) = &module[stmt].ident {
                            self.insert_entry(
                                ident.clone(),
                                ModuleScopeEntry::Stmt(pc.stmt.unwrap()),
                            );
                        } else if let StmtItem::BlockInfo(blk) = module[stmt].item {
                            let block_info = &module[blk];
                            if let Some(ident) = &block_info.ident {
                                self.insert_entry(
                                    ident.clone(),
                                    ModuleScopeEntry::Block(block_info.block_id),
                                );
                            }
                        }
                    }
                }
                ModuleItem::ModuleInst(idx) => {
                    self.collect_module_inst(module, *idx);
                }
                ModuleItem::ContinuousAssignment(_) => {}
            };
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockScope {
    pub block_id: BlockId,
    pub ident: Option<Ident>,
    // pub parent: Option<Idx<Block>>,
    pub entries: FxHashMap<Ident, BlockScopeEntry>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum BlockScopeEntry {
    SubDecl(Idx<SubDecl>),
    Block(BlockId),
    Stmt(Idx<Stmt>),
}

impl BlockScope {
    pub fn new(block_id: BlockId, ident: Option<Ident>) -> Self {
        BlockScope {
            block_id,
            ident,
            // parent,
            entries: FxHashMap::default(),
        }
    }

    pub fn block_scope_query(db: &dyn HirDb, block_id: BlockId) -> Arc<BlockScope> {
        let block = db.block(block_id);
        let mut block_scope = BlockScope::new(block_id, block.info.ident.clone());

        for item in &block.data.block_item_decls {
            match item {
                BlockItemDecl::DataDecl(idx) => {
                    let data_decl = &block[*idx];
                    let sub_decls = match data_decl {
                        DataDecl::NetDecl(net_decl) => &net_decl.sub_decls,
                        DataDecl::ParamDecl(param_decl) => &param_decl.sub_decls,
                        DataDecl::VarDecl(var_decl) => &var_decl.sub_decls,
                    }
                    .clone();
                    for idx in sub_decls {
                        let sub_decl = &block[idx];
                        let ident = sub_decl.ident.clone();
                        block_scope.insert_entry(ident, BlockScopeEntry::SubDecl(idx));
                    }
                }
            }
        }

        for (stmt_idx, stmt) in block.data.stmts.iter() {
            if let Some(ident) = &stmt.ident {
                block_scope.insert_entry(ident.clone(), BlockScopeEntry::Stmt(stmt_idx));
            }

            if let StmtItem::BlockInfo(idx) = stmt.item {
                if let Some(ident) = &block[idx].ident {
                    block_scope
                        .insert_entry(ident.clone(), BlockScopeEntry::Block(block[idx].block_id));
                }
            }
        }

        Arc::new(block_scope)
    }
}
