use base_db::{intern::Lookup, source_db::SourceDb};
use hir::{
    container::{ContainerId, InContainer, InFile, InModule},
    db::HirDb,
    hir_def::{
        block::{BlockId, BlockLoc},
        expr::declarator::DeclId,
        module::{ModuleId, instantiation::InstanceId, port::NonAnsiPortId},
        stmt::StmtId,
    },
    source_map::{IsSrc, ToAstNode},
};
use ide_db::root_db::RootDb;
use line_index::TextRange;
use smol_str::SmolStr;
use syntax::{has_name::HasName, has_text_range::HasTextRange};
use utils::get::{Get, GetRef};
use vfs::FileId;

use crate::{SymbolKind, definitions::DefinitionSource};

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

pub(crate) trait ToNav {
    fn to_nav(&self, db: &RootDb) -> NavTarget;
}

impl ToNav for DefinitionSource {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        match self {
            DefinitionSource::ModuleId(module_id) => module_id.to_nav(db),
            DefinitionSource::BlockId(block_id) => block_id.to_nav(db),
            DefinitionSource::NonAnsiPort(nonansi_port_id) => nonansi_port_id.to_nav(db),
            DefinitionSource::Decl(decl_id) => decl_id.to_nav(db),
            DefinitionSource::Instance(instance_id) => instance_id.to_nav(db),
            DefinitionSource::Stmt(stmt_id) => stmt_id.to_nav(db),
        }
    }
}

impl ToNav for ModuleId {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InFile { value: local_module_id, cont_id: file_id } = *self;
        let tree = db.parse(file_id);
        let (file, file_src_map) = db.hir_file_with_source_map(file_id);
        let decl_node = file_src_map.get(local_module_id).to_node(&tree).unwrap();

        let name = file.get(local_module_id).name.clone();
        build_nav_target(file_id.file_id(), decl_node, name, None)
    }
}

impl ToNav for BlockId {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let BlockLoc { cont_id, src: InFile { value: src, cont_id: file_id } } = self.lookup(db);
        let tree = db.parse(file_id);
        let block_node = src.to_node(&tree).unwrap();

        let (name, container_name) = match cont_id {
            ContainerId::HirFileId(file_id) => {
                let (file, file_src_map) = db.hir_file_with_source_map(file_id);
                let name = src.hir(&file, &file_src_map).name.clone();
                (name, None)
            }
            ContainerId::ModuleId(module_id) => {
                let (module, module_src_map) = db.module_with_source_map(module_id);
                let name = src.hir(&module, &module_src_map).name.clone();
                (name, module.name.clone())
            }
            ContainerId::BlockId(block_id) => {
                let (block, block_src_map) = db.block_with_source_map(block_id);
                let name = src.hir(&block, &block_src_map).name.clone();
                (name, block.name.clone())
            }
        };

        build_nav_target(file_id.file_id(), block_node, name, container_name)
    }
}

impl ToNav for InModule<NonAnsiPortId> {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InModule { value: port_id, cont_id: module_id } = *self;

        let (module, module_src_map) = db.module_with_source_map(module_id);

        let file_id = module_id.cont_id;
        let tree = db.parse(file_id);
        let port_node = module_src_map.get(port_id).to_node(&tree).unwrap();

        let name = module.get(port_id).label.clone();
        let container_name = module.name.clone();
        build_nav_target(file_id.file_id(), port_node, name, container_name)
    }
}

impl ToNav for InContainer<DeclId> {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InContainer { value: decl_id, cont_id } = *self;
        let file_id = cont_id.file_id(db);
        let tree = db.parse_src(cont_id.file_id(db));

        let (name, decl_node, container_name) = match cont_id {
            ContainerId::HirFileId(file_id) => {
                let (file, file_src_map) = db.hir_file_with_source_map(file_id);
                let name = file.get(decl_id).name.clone();
                let decl_node = file_src_map.get(decl_id).to_node(&tree).unwrap();
                (name, decl_node, None)
            }
            ContainerId::ModuleId(module_id) => {
                let (module, module_src_map) = db.module_with_source_map(module_id);
                let name = module.get(decl_id).name.clone();
                let decl_node = module_src_map.get(decl_id).to_node(&tree).unwrap();
                (name, decl_node, module.name.clone())
            }
            ContainerId::BlockId(block_id) => {
                let (block, block_src_map) = db.block_with_source_map(block_id);
                let name = block.get(decl_id).name.clone();
                let decl_node = block_src_map.get(decl_id).to_node(&tree).unwrap();
                (name, decl_node, block.name.clone())
            }
        };

        build_nav_target(file_id, decl_node, name, container_name)
    }
}

impl ToNav for InModule<InstanceId> {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InModule { value: instance_id, cont_id: module_id } = *self;
        let file_id = module_id.file_id();
        let tree = db.parse_src(file_id);

        let (module, module_src_map) = db.module_with_source_map(module_id);
        let instance_node = module_src_map.get(instance_id).to_node(&tree).unwrap();

        let name = module.get(instance_id).name.clone();
        let container_name = module.name.clone();
        build_nav_target(file_id, instance_node, name, container_name)
    }
}

impl ToNav for InContainer<StmtId> {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InContainer { value: stmt_id, cont_id } = *self;
        let file_id = cont_id.file_id(db);
        let tree = db.parse_src(file_id);

        let (name, stmt_node, container_name) = match cont_id {
            ContainerId::HirFileId(file_id) => {
                let (file, file_src_map) = db.hir_file_with_source_map(file_id);
                let name = file.get(stmt_id).label.clone();
                let stmt_node = file_src_map.get(stmt_id).to_node(&tree).unwrap();
                (name, stmt_node, None)
            }
            ContainerId::ModuleId(module_id) => {
                let (module, module_src_map) = db.module_with_source_map(module_id);
                let name = module.get(stmt_id).label.clone();
                let stmt_node = module_src_map.get(stmt_id).to_node(&tree).unwrap();
                (name, stmt_node, module.name.clone())
            }
            ContainerId::BlockId(block_id) => {
                let (block, block_src_map) = db.block_with_source_map(block_id);
                let name = block.get(stmt_id).label.clone();
                let stmt_node = block_src_map.get(stmt_id).to_node(&tree).unwrap();
                (name, stmt_node, block.name.clone())
            }
        };

        build_nav_target(file_id, stmt_node, name, container_name)
    }
}

#[inline]
fn build_nav_target<'a>(
    file_id: FileId,
    node: impl HasName<'a>,
    name: Option<SmolStr>,
    container_name: Option<SmolStr>,
) -> NavTarget {
    NavTarget {
        file_id,
        full_range: node.syntax().text_range().unwrap(),
        focus_range: node.name().and_then(|name| name.text_range()),
        name,
        kind: Some(SymbolKind::from_node(node.syntax())),
        container_name,
        description: None,
    }
}
