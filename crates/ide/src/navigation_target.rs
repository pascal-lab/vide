use base_db::intern::Lookup;
use hir::{
    container::{ContainerId, InContainer, InFile, InModule},
    db::HirDb,
    hir_def::{
        block::BlockId, data::SubDecl, module::module_item::HierarchicalInst, stmt::Stmt, ModuleId,
    },
    try_match,
};
use ide_db::root_db::RootDb;
use la_arena::Idx;
use line_index::TextRange;
use smol_str::SmolStr;
use syntax::ast::AstNode;
use utils::text_edit::to_text_range;
use vfs::vfs::FileId;

use crate::definitions::Definition;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NavTarget {
    pub file_id: FileId,
    pub full_range: TextRange,
    pub focus_range: Option<TextRange>,

    pub name: Option<SmolStr>,
    pub kind: Option<SymbolKind>,
    pub container_name: Option<SmolStr>,
    // TODO: how to represent this?
    pub description: Option<String>,
}

impl NavTarget {
    pub fn focus_or_full_range(&self) -> TextRange {
        self.focus_range.unwrap_or(self.full_range)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SymbolKind {
    Module,
    Block,
    Data,
    HierarchicalInst,
}

pub(crate) trait ToNav {
    fn to_nav(&self, db: &RootDb) -> NavTarget;
}

impl ToNav for Definition {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        match self {
            Definition::ModuleId(module_id) => module_id.to_nav(db),
            Definition::BlockId(block_id) => block_id.to_nav(db),
            Definition::HierarchyInst(inst_id) => inst_id.to_nav(db),
            Definition::SubDecl(sub_decl_id) => sub_decl_id.to_nav(db),
            Definition::Stmt(stmt_id) => stmt_id.to_nav(db),
        }
    }
}

impl ToNav for ModuleId {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InFile { value: module_info_id, file_id } = *self;
        let module = db.module(*self);
        let (node_range, name_node_range) = {
            let (_, file_src_map) = db.hir_file_with_source_map(file_id);
            let tree = db.hir_syntax_tree(file_id).unwrap();
            let module_decl = file_src_map[module_info_id].value.to_node(tree.tree()).unwrap();

            let ident_range = try_match! {
                module_decl.module_ansi_header(), header => {
                    header.identifier().unwrap().syntax().range()
                },
                module_decl.module_nonansi_header(), header => {
                    header.identifier().unwrap().syntax().range()
                },
                _ => unreachable!(),
            };
            (module_decl.syntax().range(), ident_range)
        };

        NavTarget {
            file_id: file_id.0,
            full_range: to_text_range(node_range),
            focus_range: Some(to_text_range(name_node_range)),
            name: Some(module.ident.clone()),
            kind: Some(SymbolKind::Module),
            container_name: None,
            description: None,
        }
    }
}

impl ToNav for BlockId {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InFile { value: block_src, file_id } = self.lookup(db).block_src;
        let block = db.block(*self);

        let (node_range, name_node_range) = {
            use hir::hir_def::block::block_src::LocalBlockSrc;
            let tree = db.hir_syntax_tree(file_id).unwrap();
            match block_src {
                LocalBlockSrc::SeqBlock(ptr) => {
                    let block = ptr.to_node(tree.tree()).unwrap();
                    let ident = block.identifiers().next();
                    (block.syntax().range(), ident.map(|it| it.syntax().range()))
                }
                LocalBlockSrc::ParBlock(ptr) => {
                    let block = ptr.to_node(tree.tree()).unwrap();
                    let ident = block.identifiers().next();
                    (block.syntax().range(), ident.map(|it| it.syntax().range()))
                }
            }
        };

        NavTarget {
            file_id: file_id.0,
            full_range: to_text_range(node_range),
            focus_range: name_node_range.map(to_text_range),
            name: block.info.ident.clone(),
            kind: Some(SymbolKind::Block),
            container_name: None,
            description: None,
        }
    }
}

impl ToNav for InModule<Idx<HierarchicalInst>> {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InModule { value: inst_id, module_id } = *self;
        let InFile { file_id, .. } = module_id;
        let module = db.module(module_id);

        let (node_range, name_node_range) = {
            let (_, module_src_map) = db.module_with_source_map(module_id);
            let tree = db.hir_syntax_tree(file_id).unwrap();
            let inst = module_src_map[inst_id].value.to_node(tree.tree()).unwrap();
            let ident_range = inst
                .name_of_instance()
                .and_then(|it| it.identifier().map(|it| it.syntax().range()));
            (inst.syntax().range(), ident_range)
        };

        NavTarget {
            file_id: file_id.0,
            full_range: to_text_range(node_range),
            focus_range: name_node_range.map(to_text_range),
            name: Some(module.ident.clone()),
            kind: Some(SymbolKind::HierarchicalInst),
            container_name: None,
            description: None,
        }
    }
}

impl ToNav for InContainer<Idx<SubDecl>> {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InContainer { value: sub_decl_id, container_id } = *self;
        let file_id = match container_id {
            ContainerId::HirFileId(file_id) => file_id,
            ContainerId::ModuleId(module_id) => module_id.file_id,
            ContainerId::BlockId(block_id) => block_id.lookup(db).block_src.file_id,
        };

        let (node_range, name_node_range, name) = {
            use hir::hir_def::data::LocalSubDeclSrc;
            let tree = db.hir_syntax_tree(file_id).unwrap();
            let (name, src) = match container_id {
                ContainerId::HirFileId(_) => unreachable!(),
                ContainerId::ModuleId(module_id) => {
                    let (module, module_src_map) = db.module_with_source_map(module_id);
                    let src = module_src_map[sub_decl_id].value.clone();
                    (module[sub_decl_id].ident.clone(), src)
                }
                ContainerId::BlockId(block_id) => {
                    let (block, block_src_map) = db.block_with_source_map(block_id);
                    let src = block_src_map[sub_decl_id].value.clone();
                    (block[sub_decl_id].ident.clone(), src)
                }
            };
            let (node_range, name_node_range) = match src {
                LocalSubDeclSrc::NetDeclAssign(ptr) => {
                    let node = ptr.to_node(tree.tree()).unwrap();
                    (node.syntax().range(), node.identifier().unwrap().syntax().range())
                }
                LocalSubDeclSrc::VarDeclAssign(ptr) => {
                    let node = ptr.to_node(tree.tree()).unwrap();
                    (node.syntax().range(), node.identifier().unwrap().syntax().range())
                }
                LocalSubDeclSrc::ParamAssign(ptr) => {
                    let node = ptr.to_node(tree.tree()).unwrap();
                    (node.syntax().range(), node.identifier().unwrap().syntax().range())
                }
                LocalSubDeclSrc::AnsiPortDecl(ptr) => {
                    let node = ptr.to_node(tree.tree()).unwrap();
                    (node.syntax().range(), node.identifier().unwrap().syntax().range())
                }
                LocalSubDeclSrc::PortIdentDecl(ptr) => {
                    let node = ptr.to_node(tree.tree()).unwrap();
                    (node.syntax().range(), node.identifier().unwrap().syntax().range())
                }
                LocalSubDeclSrc::VarIdentDecl(ptr) => {
                    let node = ptr.to_node(tree.tree()).unwrap();
                    (node.syntax().range(), node.identifier().unwrap().syntax().range())
                }
                LocalSubDeclSrc::VarPortIdentDecl(ptr) => {
                    let node = ptr.to_node(tree.tree()).unwrap();
                    (node.syntax().range(), node.identifier().unwrap().syntax().range())
                }
            };
            (node_range, name_node_range, name)
        };

        NavTarget {
            file_id: file_id.0,
            full_range: to_text_range(node_range),
            focus_range: Some(to_text_range(name_node_range)),
            name: Some(name),
            kind: Some(SymbolKind::Data),
            container_name: None,
            description: None,
        }
    }
}

impl ToNav for InContainer<Idx<Stmt>> {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InContainer { value: stmt_id, container_id } = *self;
        let file_id = match container_id {
            ContainerId::HirFileId(file_id) => file_id,
            ContainerId::ModuleId(module_id) => module_id.file_id,
            ContainerId::BlockId(block_id) => block_id.lookup(db).block_src.file_id,
        };

        let (node_range, name_node_range, name) = {
            let tree = db.hir_syntax_tree(file_id).unwrap();
            let (name, ptr) = match container_id {
                ContainerId::HirFileId(_) => unreachable!(),
                ContainerId::ModuleId(module_id) => {
                    let (module, module_src_map) = db.module_with_source_map(module_id);
                    let ptr = module_src_map[stmt_id].value.clone();
                    (module[stmt_id].ident.clone(), ptr)
                }
                ContainerId::BlockId(block_id) => {
                    let (block, block_src_map) = db.block_with_source_map(block_id);
                    let ptr = block_src_map[stmt_id].value.clone();
                    (block[stmt_id].ident.clone(), ptr)
                }
            };
            let node = ptr.to_node(tree.tree()).unwrap();
            let node_range = node.syntax().range();
            let name_node_range = node.identifier().map(|it| it.syntax().range());
            (node_range, name_node_range, name)
        };

        NavTarget {
            file_id: file_id.0,
            full_range: to_text_range(node_range),
            focus_range: name_node_range.map(to_text_range),
            name,
            kind: Some(SymbolKind::Data),
            container_name: None,
            description: None,
        }
    }
}
