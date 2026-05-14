use rustc_hash::FxHashMap;
use syntax::{
    SyntaxAncestors, SyntaxNode,
    ast::{self, AstNode},
    match_ast,
};
use utils::get::{Get, GetRef};

use super::hir_to_def::Hir2DefCache;
use crate::{
    container::{ContainerId, InFile},
    db::HirDb,
    file::HirFileId,
    hir_def::{
        block::{BlockId, BlockSrc, find_local_block_id},
        module::{
            ModuleId, ModuleSrc,
            generate::{GenerateBlockLoc, GenerateBlockSrc},
        },
        subroutine::{SubroutineLoc, SubroutineSrc},
    },
    source_map::ToAstNode,
};

#[derive(Default, Debug)]
pub(super) struct Source2DefCache<'db> {
    container_map: FxHashMap<InFile<SyntaxNode<'db>>, ContainerId>,
}

pub(super) struct Source2DefCtx<'db, 'cache> {
    pub(super) db: &'db dyn HirDb,
    pub(super) source_cache: &'cache mut Source2DefCache<'db>,
    pub(super) hir_cache: &'cache mut Hir2DefCache,
}

impl Source2DefCtx<'_, '_> {
    pub(super) fn module_to_def(
        &mut self,
        InFile { file_id, value: src }: InFile<ModuleSrc>,
    ) -> Option<ModuleId> {
        let (_, file_source_map) = self.db.hir_file_with_source_map(file_id);
        Some(ModuleId::new(file_id, file_source_map.get(src)))
    }

    pub(super) fn block_to_def(
        &mut self,
        InFile { file_id, value: block_src }: InFile<BlockSrc>,
    ) -> Option<BlockId> {
        let tree = self.db.parse(file_id);
        let node = block_src.to_node(&tree)?;
        self.block_to_def_inner(file_id, node, block_src)
    }

    // This is a faster version of block_to_def that doesn't require a [`to_node`]
    fn block_to_def_inner(
        &mut self,
        file_id: HirFileId,
        block: ast::BlockStatement,
        block_src: BlockSrc,
    ) -> Option<BlockId> {
        let node = block.syntax();
        let container = self.find_container(InFile::new(file_id, node));

        let block_id = match container {
            ContainerId::HirFileId(file_id) => {
                let (file, file_src_map) = self.db.hir_file_with_source_map(file_id);
                let local_block_id = find_local_block_id(&file_src_map.stmt_srcs, block_src)?;
                file.get(local_block_id).block_id
            }
            ContainerId::ModuleId(module_id) => {
                let (module, module_src_map) = self.db.module_with_source_map(module_id);
                let local_block_id = find_local_block_id(&module_src_map.stmt_srcs, block_src)?;
                module.get(local_block_id).block_id
            }
            ContainerId::BlockId(block_id) => {
                let (block, block_src_map) = self.db.block_with_source_map(block_id);
                let local_block_id = *block_src_map.block_srcs.get(&block_src)?;
                block.get(local_block_id).block_id
            }
            ContainerId::GenerateBlockId(generate_block_id) => {
                let (generate_block, generate_block_src_map) =
                    self.db.generate_block_with_source_map(generate_block_id);
                let local_block_id = generate_block_src_map.get(block_src);
                generate_block.get(local_block_id).block_id
            }
            ContainerId::SubroutineId(subroutine_id) => {
                let (subroutine, subroutine_src_map) =
                    self.db.subroutine_with_source_map(subroutine_id);
                let local_block_id = *subroutine_src_map.block_srcs.get(&block_src)?;
                subroutine.stmts.get(local_block_id).block_id
            }
        };

        Some(block_id)
    }

    fn container_to_def(&mut self, file_id: HirFileId, node: SyntaxNode) -> Option<ContainerId> {
        let cont_id = match_ast! { node,
           ast::ModuleDeclaration[module] => {
               let src = module.into();
               self.module_to_def(InFile::new(file_id, src))?.into()
           },
           ast::BlockStatement[block] => {
               let block_src = BlockSrc::from(block);
               self.block_to_def_inner(file_id, block, block_src)?.into()
           },
           ast::GenerateBlock[block] => {
               let src = GenerateBlockSrc::from_generate_block(block);
               let anchor = match src {
                   GenerateBlockSrc::GenerateBlock { .. } => block.syntax(),
                   GenerateBlockSrc::LoopGenerate { .. } => block.syntax().parent()?,
               };
               let parent = SyntaxAncestors::start_from(anchor)
                   .skip(1)
                   .find_map(|node| self.container_to_def(file_id, node))
                   .unwrap_or(file_id.into());
               self.db.intern_generate_block(GenerateBlockLoc {
                   cont_id: parent,
                   src: InFile::new(file_id, src),
               }).into()
           },
           ast::FunctionDeclaration[func] => {
               let mut ancestors = SyntaxAncestors::start_from(node).skip(1);
               let module_id = ancestors.find_map(|ancestor| match_ast! { ancestor,
                   ast::ModuleDeclaration[module] => {
                       let src = ModuleSrc::from(module);
                       self.module_to_def(InFile::new(file_id, src))
                   },
                   _ => None,
               });

               let cont_id: ContainerId = match module_id {
                   Some(module_id) => module_id.into(),
                   None => file_id.into(),
               };

               let src = SubroutineSrc::from(func);
               let subroutine_id = self.db.intern_subroutine(SubroutineLoc {
                   cont_id,
                   src: InFile::new(file_id, src),
               });
               subroutine_id.into()
           },
           ast::CompilationUnit => file_id.into(),
           _ => return None,
        };

        Some(cont_id)
    }

    pub(super) fn find_container(
        &mut self,
        InFile { value: node, file_id }: InFile<SyntaxNode>,
    ) -> ContainerId {
        let node = unsafe { std::mem::transmute::<SyntaxNode<'_>, SyntaxNode<'_>>(node) };
        let in_file = InFile::new(file_id, node);

        if let Some(container_id) = self.source_cache.container_map.get(&in_file) {
            return *container_id;
        }

        let container_id = SyntaxAncestors::start_from(node)
            .skip(1) // skip the node itself
            .find_map(|node| self.container_to_def(file_id, node))
            .unwrap_or(file_id.into());
        self.source_cache.container_map.insert(in_file, container_id);
        container_id
    }
}
