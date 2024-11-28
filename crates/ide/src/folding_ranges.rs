use hir::{
    db::HirDb,
    doc_tree::DocTree,
    file::HirFileId,
    hir_def::{
        block::{BlockId, BlockSrc},
        module::{ModuleId, ModuleSrc},
        stmt::{Stmt, StmtKind, StmtSrc},
    },
    source_map::{IsNamedSrc, IsSrc, SourceMap},
};
use ide_db::{
    line_index_db::{LineIndexDb, LineIndexExt},
    root_db::RootDb,
};
use la_arena::Arena;
use line_index::{LineIndex, TextRange};
use utils::get::{Get, GetRef};
use vfs::FileId;

#[derive(Debug, Clone, Copy)]
pub struct FoldingConfig {
    pub line_fold_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldKind {
    Comment,
    Imports, // TODO: fold macros
    Region,
    Module,
    PortList,
    Decl,
    Declaration,
    ContAssign,
    Instance,
    Stmt,
    Block,
}

#[derive(Debug)]
pub struct Fold {
    pub range: TextRange,
    pub kind: FoldKind,
    pub collapsed_text: Option<String>,
}

impl Fold {
    fn new(range: TextRange, kind: FoldKind) -> Self {
        Self { range, kind, collapsed_text: None }
    }

    #[inline]
    fn try_build(range: TextRange, kind: FoldKind, line_index: &LineIndex) -> Option<Self> {
        (line_index.line_ranges(range).len() > 1).then(|| Self::new(range, kind))
    }
}

trait FoldCollector {
    fn collect_folds<Src: IsSrc, Hir>(
        &mut self,
        srcs: &SourceMap<Src, Hir>,
        kind: FoldKind,
        line_index: &LineIndex,
    );

    fn collect_fold(&mut self, src: impl IsSrc, kind: FoldKind, line_index: &LineIndex);

    fn collect_docs(&mut self, docs: &DocTree, line_index: &LineIndex);
}

impl FoldCollector for Vec<Fold> {
    #[inline]
    fn collect_folds<Src: IsSrc, Hir>(
        &mut self,
        srcs: &SourceMap<Src, Hir>,
        kind: FoldKind,
        line_index: &LineIndex,
    ) {
        self.extend(
            srcs.iter().filter_map(|(_, src)| Fold::try_build(src.range(), kind, line_index)),
        );
    }

    #[inline]
    fn collect_fold(&mut self, src: impl IsSrc, kind: FoldKind, line_index: &LineIndex) {
        if let Some(fold) = Fold::try_build(src.range(), kind, line_index) {
            self.push(fold);
        }
    }

    #[inline]
    fn collect_docs(&mut self, docs: &DocTree, line_index: &LineIndex) {
        self.extend(
            docs.nodes
                .values()
                .filter_map(|node| Fold::try_build(node.range, FoldKind::Region, line_index)),
        );
    }
}

pub(crate) fn folding_ranges(db: &RootDb, file_id: FileId, _config: &FoldingConfig) -> Vec<Fold> {
    let line_index = db.line_index(file_id);
    let line_index = line_index.as_ref();

    let file_id = HirFileId(file_id);
    let (file, src_map) = db.hir_file_with_source_map(file_id);

    let mut folds = Vec::default();

    folds.collect_docs(&src_map.doc_tree, line_index);

    src_map.module_srcs.iter().for_each(|(idx, src)| {
        collect_module(db, &mut folds, ModuleId::new(file_id, idx), *src, line_index)
    });

    folds.collect_folds(&src_map.declaration_srcs, FoldKind::Declaration, line_index);
    folds.collect_folds(&src_map.decl_srcs, FoldKind::Decl, line_index);
    collect_stmt(db, &mut folds, &file.stmts, &src_map.stmt_srcs, line_index);

    folds
}

fn collect_module(
    db: &RootDb,
    folds: &mut Vec<Fold>,
    module_id: ModuleId,
    module_src: ModuleSrc,
    line_index: &LineIndex,
) {
    let (module, src_map) = db.module_with_source_map(module_id);

    folds.collect_docs(&src_map.doc_tree, line_index);

    if let Some(port_list_src) = src_map.port_list_src {
        let port_list_fold = Fold::try_build(port_list_src.range(), FoldKind::PortList, line_index);
        let module_body_start = port_list_fold
            .as_ref()
            .and_then(|port_list| {
                let line = line_index.line_col(port_list.range.end()).line + 1;
                // TODO: line maybe out of range
                line_index.line(line)
            })
            .unwrap_or(module_src.range());

        folds.extend(port_list_fold);

        let module_range = TextRange::new(module_body_start.start(), module_src.range().end());
        folds.extend(Fold::try_build(module_range, FoldKind::Module, line_index));
    } else {
        folds.collect_fold(module_src, FoldKind::Module, line_index);
    }

    folds.collect_folds(&src_map.assign_srcs, FoldKind::ContAssign, line_index);
    folds.collect_folds(&src_map.declaration_srcs, FoldKind::Declaration, line_index);
    folds.collect_folds(&src_map.decl_srcs, FoldKind::Decl, line_index);

    folds.extend(src_map.instance_srcs.iter().filter_map(|(instance_id, src)| {
        let instantiation_id = module.get(instance_id).parent;

        if module.get(instantiation_id).instances.len() > 1 {
            let range = src.range();
            let start = src.name_range().map_or(range.start(), |r| r.end());
            Fold::try_build(TextRange::new(start, range.end()), FoldKind::Instance, line_index)
        } else {
            let instantiation_src = src_map.get(instantiation_id);
            Fold::try_build(instantiation_src.range(), FoldKind::Instance, line_index)
        }
    }));

    collect_stmt(db, folds, &module.stmts, &src_map.stmt_srcs, line_index);
}

fn collect_block(
    db: &RootDb,
    folds: &mut Vec<Fold>,
    block_id: BlockId,
    block_src: BlockSrc,
    line_index: &LineIndex,
) {
    let (block, src_map) = db.block_with_source_map(block_id);

    folds.collect_docs(&src_map.doc_tree, line_index);

    folds.collect_fold(block_src, FoldKind::Block, line_index);
    folds.collect_folds(&src_map.declaration_srcs, FoldKind::Declaration, line_index);
    folds.collect_folds(&src_map.decl_srcs, FoldKind::Decl, line_index);
    collect_stmt(db, folds, &block.stmts, &src_map.stmt_srcs, line_index)
}

fn collect_stmt(
    db: &RootDb,
    folds: &mut Vec<Fold>,
    arena: &Arena<Stmt>,
    src_map: &SourceMap<StmtSrc, Stmt>,
    line_index: &LineIndex,
) {
    src_map.iter().for_each(|(stmt_id, &stmt_src)| match &arena.get(stmt_id).kind {
        StmtKind::Block(block_info) => {
            let block_src = stmt_src.try_into().unwrap();
            collect_block(db, folds, block_info.block_id, block_src, line_index);
        }
        _ => {
            folds.collect_fold(stmt_src, FoldKind::Stmt, line_index);
        }
    });
}
