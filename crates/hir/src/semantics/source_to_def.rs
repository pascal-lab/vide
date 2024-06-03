use syntax::{
    ast::{self, ptr::AstNodePtr, AstNode},
    syntax_kind, SyntaxAncestors, SyntaxNode,
};

use crate::{
    container::ContainerId,
    container::InFile,
    db::HirDb,
    hir_def::{
        block::{
            block_src::{BlockSrc, LocalBlockSrc},
            BlockId,
        },
        FileItem, LocalModuleSrc, ModuleId, ModuleSrc,
    },
};

pub(super) struct Source2DefCtx<'a> {
    pub(super) db: &'a dyn HirDb,
}

impl Source2DefCtx<'_> {
    pub(super) fn module_to_def(
        &mut self,
        module_src: &InFile<LocalModuleSrc>,
    ) -> Option<ModuleId> {
        let file_id = module_src.file_id;
        let (_, file_source_map) = self.db.hir_file_with_source_map(file_id);
        file_source_map
            .modules
            .get_idx(module_src)
            .map(|module_id| ModuleId::new(file_id, *module_id))
    }

    pub(super) fn block_to_def(&mut self, block_src: &InFile<LocalBlockSrc>) -> Option<BlockId> {
        let tree = self.db.syntax_tree(block_src.file_id.0)?;
        let node = block_src.value.syntax().to_node(tree.tree())?;
        let container = self.find_container(block_src.clone().with_value(node))?;

        match container {
            ContainerId::HirFileId(_) => unreachable!(),
            ContainerId::ModuleId(module_id) => {
                let (module, module_src_map) = self.db.module_with_source_map(module_id);
                let block_info_id = module_src_map.block.src2hir.get(block_src)?;
                Some(module[*block_info_id].block_id)
            }
            ContainerId::BlockId(block_id) => {
                let (block, block_src_map) = self.db.block_with_source_map(block_id);
                let block_info_id = block_src_map.block.src2hir.get(block_src)?;
                Some(block[*block_info_id].block_id)
            }
        }
    }

    fn container_to_def(
        &mut self,
        InFile { file_id, value: node }: InFile<SyntaxNode>,
    ) -> Option<ContainerId> {
        let cont_id = match node.kind_id() {
            syntax_kind::MODULE_DECLARATION => {
                let value = ast::ModuleDeclaration::cast(node).unwrap().to_ptr();
                let module_src = ModuleSrc { file_id, value };
                self.module_to_def(&module_src)?.into()
            }
            syntax_kind::SEQ_BLOCK | syntax_kind::PAR_BLOCK => {
                let value = LocalBlockSrc::cast(node.into()).unwrap();
                let block_src = BlockSrc { file_id, value };
                self.block_to_def(&block_src)?.into()
            }
            _ => return None,
        };
        Some(cont_id)
    }

    pub(crate) fn find_container(&mut self, src: InFile<SyntaxNode>) -> Option<ContainerId> {
        for container in SyntaxAncestors::new(&src.value) {
            if let Some(def) = self.container_to_def(src.with_value(container)) {
                return Some(def);
            }
        }

        let (file, _) = self.db.hir_file_with_source_map(src.file_id);
        let container = match file.items.first()? {
            FileItem::Module(module_id) => src.with_value(*module_id).into(),
        };
        Some(container)
    }
}
