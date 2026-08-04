use hir_def::{
    block::{BlockId, BlockSrc, find_local_block_id},
    container::{ArenaOwnerId, InFile, SubroutineParent, SubroutineScope},
    db::HirDefDb,
    module::{
        ModuleId, ModuleSrc,
        generate::{GenerateBlockLoc, GenerateBlockSrc},
    },
    source_map::ToAstNode,
    subroutine::{LocalSubroutineId, SubroutineSrc},
};
use preproc_expand::file::HirFileId;
use syntax::{
    SyntaxAncestors, SyntaxNode,
    ast::{self, AstNode},
    match_ast,
};

pub(super) fn module_to_def(
    db: &dyn HirDefDb,
    InFile { file_id, value: src }: InFile<ModuleSrc>,
) -> Option<ModuleId> {
    let file = db.hir_file_with_source_map(file_id);
    Some(ModuleId::new(file_id, file.hir_id(src)?))
}

pub(super) fn block_to_def(
    db: &dyn HirDefDb,
    InFile { file_id, value: block_src }: InFile<BlockSrc>,
) -> Option<BlockId> {
    let tree = db.parse(file_id);
    let node = block_src.to_node(&tree)?;
    block_to_def_inner(db, file_id, node, block_src)
}

pub(super) fn subroutine_to_def(
    db: &dyn HirDefDb,
    InFile { file_id, value: subroutine_src }: InFile<SubroutineSrc>,
) -> Option<SubroutineScope> {
    let tree = db.parse(file_id);
    let node = subroutine_src.to_node(&tree)?;
    subroutine_to_def_inner(db, file_id, node, subroutine_src)
}

// This is a faster version of block_to_def that doesn't require a [`to_node`]
fn block_to_def_inner(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    block: ast::BlockStatement,
    block_src: BlockSrc,
) -> Option<BlockId> {
    let node = block.syntax();
    let container = find_container(db, InFile::new(file_id, node));

    let block_id = match container {
        ArenaOwnerId::File(file_id) => {
            let file = db.hir_file_with_source_map(file_id);
            let local_block_id = find_local_block_id(&file.source_map().stmt_srcs, block_src)?;
            file.get(local_block_id).block_id
        }
        ArenaOwnerId::Module(module_id) => {
            let module = db.module_with_source_map(module_id);
            let local_block_id =
                find_local_block_id(&module.source_map().stmt_srcs, block_src)?;
            module.get(local_block_id).block_id
        }
        ArenaOwnerId::Block(block_id) => {
            let block = db.block_with_source_map(block_id);
            let local_block_id = *block.source_map().block_srcs.get(&block_src)?;
            block.get(local_block_id).block_id
        }
        ArenaOwnerId::GenerateBlock(generate_block_id) => {
            let generate_block = db.generate_block_with_source_map(generate_block_id);
            let local_block_id = generate_block.hir_id(block_src)?;
            generate_block.get(local_block_id).block_id
        }
        ArenaOwnerId::Subroutine(subroutine_id) => {
            let subroutine = db.subroutine_with_source_map(subroutine_id);
            let local_block_id = *subroutine.source_map().block_srcs.get(&block_src)?;
            subroutine.get(local_block_id).block_id
        }
    };

    Some(block_id)
}

fn container_to_def(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    node: SyntaxNode,
) -> Option<ArenaOwnerId> {
    let cont_id = match_ast! { node,
       ast::ModuleDeclaration[module] => {
           let src = ModuleSrc::from_ast(file_id, module);
           module_to_def(db, InFile::new(file_id, src))?.into()
       },
       ast::BlockStatement[block] => {
           let block_src = BlockSrc::from_ast(file_id, block);
           block_to_def_inner(db, file_id, block, block_src)?.into()
       },
       ast::GenerateBlock[block] => {
           let src = GenerateBlockSrc::from_generate_block(block);
           let anchor = match src {
               GenerateBlockSrc::GenerateBlock { .. } => block.syntax(),
               GenerateBlockSrc::LoopGenerate { .. } => block.syntax().parent()?,
               GenerateBlockSrc::SingleMember { .. } => block.syntax(),
           };
           let parent = SyntaxAncestors::start_from(anchor)
               .skip(1)
               .find_map(|node| container_to_def(db, file_id, node))
               .unwrap_or(file_id.into());
           db.intern_generate_block(GenerateBlockLoc {
               cont_id: parent,
               src: InFile::new(file_id, src),
           }).into()
       },
       ast::FunctionDeclaration[func] => {
           let src = SubroutineSrc::from_ast(file_id, func);
           subroutine_to_def_inner(db, file_id, func, src)?.into()
       },
       ast::CompilationUnit => file_id.into(),
       _ => {
           let member = ast::Member::cast(node)?;
           single_member_generate_block_to_def(db, file_id, member)?
       },
    };

    Some(cont_id)
}

fn subroutine_to_def_inner(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    node: ast::FunctionDeclaration,
    src: SubroutineSrc,
) -> Option<SubroutineScope> {
    let parent = ast::Member::cast(node.syntax())
        .and_then(|member| single_member_generate_block_to_def(db, file_id, member))
        .or_else(|| {
            SyntaxAncestors::start_from(node.syntax())
                .skip(1)
                .find_map(|node| container_to_def(db, file_id, node))
        })
        .unwrap_or(file_id.into());
    let parent = match parent {
        ArenaOwnerId::File(file_id) => SubroutineParent::File(file_id),
        ArenaOwnerId::Module(module_id) => SubroutineParent::Module(module_id),
        ArenaOwnerId::GenerateBlock(generate_block_id) => {
            SubroutineParent::GenerateBlock(generate_block_id)
        }
        ArenaOwnerId::Block(_) | ArenaOwnerId::Subroutine(_) => return None,
    };
    let local_id = local_subroutine_id(db, parent, src)?;
    Some(SubroutineScope::new(parent, local_id))
}

fn local_subroutine_id(
    db: &dyn HirDefDb,
    cont_id: SubroutineParent,
    src: SubroutineSrc,
) -> Option<LocalSubroutineId> {
    match cont_id {
        SubroutineParent::File(file_id) => {
            db.hir_file_with_source_map(file_id).hir_id(src)
        }
        SubroutineParent::Module(module_id) => {
            db.module_with_source_map(module_id).hir_id(src)
        }
        SubroutineParent::GenerateBlock(generate_block_id) => {
            db.generate_block_with_source_map(generate_block_id).hir_id(src)
        }
    }
}

fn single_member_generate_block_to_def(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    member: ast::Member,
) -> Option<ArenaOwnerId> {
    if matches!(member, ast::Member::GenerateBlock(_) | ast::Member::LoopGenerate(_)) {
        return None;
    }

    let anchor = member.syntax();
    if !is_generate_branch_member(anchor) {
        return None;
    }

    let parent = SyntaxAncestors::start_from(anchor)
        .skip(1)
        .find_map(|node| container_to_def(db, file_id, node))
        .unwrap_or(file_id.into());

    Some(
        db.intern_generate_block(GenerateBlockLoc {
            cont_id: parent,
            src: InFile::new(file_id, member.into()),
        })
        .into(),
    )
}

fn is_generate_branch_member(member: SyntaxNode) -> bool {
    for ancestor in SyntaxAncestors::start_from(member).skip(1) {
        if ast::IfGenerate::can_cast(ancestor.kind())
            || ast::CaseGenerate::can_cast(ancestor.kind())
        {
            return true;
        }

        if ast::GenerateBlock::can_cast(ancestor.kind())
            || ast::GenerateRegion::can_cast(ancestor.kind())
            || ast::ModuleDeclaration::can_cast(ancestor.kind())
            || ast::BlockStatement::can_cast(ancestor.kind())
        {
            return false;
        }
    }

    false
}

pub(super) fn find_container(
    db: &dyn HirDefDb,
    InFile { value: node, file_id }: InFile<SyntaxNode>,
) -> ArenaOwnerId {
    SyntaxAncestors::start_from(node)
        .skip(1) // skip the node itself
        .find_map(|node| container_to_def(db, file_id, node))
        .unwrap_or(file_id.into())
}
