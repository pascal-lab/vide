use base_db::source_db::SourceDb;
use hir::{
    db::HirDb,
    file::HirFileId,
    hir_def::{
        block::{BlockId, BlockSrc},
        module::{ModuleId, ModuleSrc},
        stmt::{Stmt, StmtKind, StmtSrc},
    },
    region_tree::RegionTree,
    source_map::{IsNamedSrc, IsSrc, SourceMap},
};
use ide_db::{line_index_db::LineIndexDb, root_db::RootDb};
use la_arena::Arena;
use memchr::memmem::Finder;
use rustc_hash::FxHashSet;
use syntax::{
    SyntaxCursor, SyntaxCursorExt, SyntaxTrivia,
    token::SyntaxTokenExt,
    trivia::{TriviaExt, TriviaKindExt},
};
use utils::{
    get::{Get, GetRef},
    line_index::{LineIndex, TextRange},
    text_edit::TextSize,
};
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
    Config,
    PortList,
    Decl,
    Declaration,
    ContAssign,
    DefParam,
    Instance,
    Stmt,
    Block,
    Opaque,
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

    fn collect_docs(&mut self, docs: &RegionTree, line_index: &LineIndex);
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
    fn collect_docs(&mut self, docs: &RegionTree, line_index: &LineIndex) {
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

    collect_comments(db, file_id, line_index, &mut folds);

    folds.collect_docs(&src_map.region_tree, line_index);

    src_map.module_srcs.iter().for_each(|(idx, src)| {
        collect_module(db, &mut folds, ModuleId::new(file_id, idx), *src, line_index)
    });

    folds.collect_folds(&src_map.config_decl_srcs, FoldKind::Config, line_index);
    folds.collect_folds(&src_map.declaration_srcs, FoldKind::Declaration, line_index);
    folds.collect_folds(&src_map.decl_srcs, FoldKind::Decl, line_index);
    folds.collect_folds(&src_map.opaque_srcs, FoldKind::Opaque, line_index);
    collect_stmt(db, &mut folds, &file.stmts, &src_map.stmt_srcs, line_index);

    folds
}

fn collect_comments(
    db: &RootDb,
    file_id: HirFileId,
    line_index: &LineIndex,
    folds: &mut Vec<Fold>,
) {
    let tree = db.parse(file_id);
    let Some(root) = tree.root() else {
        return;
    };
    let mut cursor = root.walk();

    let text = db.file_text(file_id.file_id());
    let visited_ranges = collect_line_comments(&text, &mut cursor, line_index, folds);
    collect_block_comments(&text, &mut cursor, line_index, visited_ranges, folds);
}

fn collect_line_comments(
    text: &str,
    cursor: &mut SyntaxCursor<'_>,
    line_index: &LineIndex,
    folds: &mut Vec<Fold>,
) -> FxHashSet<usize> {
    let finder = Finder::new("//");
    let it = finder.find_iter(text.as_bytes());
    let mut last_pos = 0;

    let mut visited_ranges = FxHashSet::default();

    for start in it {
        if start < last_pos {
            continue;
        }

        cursor.reset_to_root();
        cursor.goto_first_tok_after_or_last(TextSize::from(start as u32));
        let tok = cursor.to_token().unwrap();
        visited_ranges.insert(tok.range().unwrap().start());
        let mut trivias = tok.trivias_with_range().peekable();

        let check_lc = |t: &SyntaxTrivia| {
            t.kind().is_lc() && t.is_region_begin().is_none() && !t.is_region_end()
        };

        // (1 eol + 1 whitespace (optional) + 1 line comment){>=2}
        while let Some((range, t)) = trivias.next() {
            if check_lc(&t) {
                let comment_start = range.start();
                let mut comment_end = None;

                while trivias.next_if(|(_, t)| t.kind().is_eol()).is_some() {
                    trivias.next_if(|(_, t)| t.kind().is_whitespace());

                    if let Some((range, _)) = trivias.next_if(|(_, t)| check_lc(t)) {
                        comment_end = Some(range.end());
                    } else {
                        break;
                    }
                }

                if let Some(comment_end) = comment_end {
                    let range = TextRange::new(comment_start, comment_end);
                    let fold = Fold::try_build(range, FoldKind::Comment, line_index).unwrap();
                    folds.push(fold);
                }
            } else if t.kind().is_bc() {
                let range = TextRange::new(range.start(), range.end());
                let fold = Fold::try_build(range, FoldKind::Comment, line_index).unwrap();
                folds.push(fold);
            }
        }

        last_pos = tok.range().unwrap().start();
    }

    visited_ranges
}

fn collect_block_comments(
    text: &str,
    cursor: &mut SyntaxCursor<'_>,
    line_index: &LineIndex,
    visited_ranges: FxHashSet<usize>,
    folds: &mut Vec<Fold>,
) {
    let finder = Finder::new("/*");
    let it = finder.find_iter(text.as_bytes());
    let mut last_pos = 0;

    for start in it {
        if start < last_pos || visited_ranges.contains(&start) {
            continue;
        }

        cursor.reset_to_root();
        cursor.goto_first_tok_after_or_last(TextSize::from(start as u32));
        let tok = cursor.to_token().unwrap();

        let trivias = tok.trivias_with_range();
        for (range, trivia) in trivias {
            if !trivia.kind().is_bc() {
                continue;
            }

            if let Some(fold) = Fold::try_build(range, FoldKind::Comment, line_index) {
                folds.push(fold);
            }
        }

        last_pos = tok.range().unwrap().start();
    }
}

fn collect_module(
    db: &RootDb,
    folds: &mut Vec<Fold>,
    module_id: ModuleId,
    module_src: ModuleSrc,
    line_index: &LineIndex,
) {
    let (module, src_map) = db.module_with_source_map(module_id);

    folds.collect_docs(&src_map.region_tree, line_index);

    if let Some(port_list_src) = src_map.port_srcs.port_list_src() {
        let port_list_fold = Fold::try_build(port_list_src.range(), FoldKind::PortList, line_index);
        let module_body_start = port_list_fold
            .as_ref()
            .and_then(|port_list| {
                let line = line_index.line_col(port_list.range.end()).line + 1;
                line_index.range_for_line(line.min(line_index.lines_len().saturating_sub(1)))
            })
            .unwrap_or(module_src.range());

        folds.extend(port_list_fold);

        let module_range = TextRange::new(module_body_start.start(), module_src.range().end());
        folds.extend(Fold::try_build(module_range, FoldKind::Module, line_index));
    } else {
        folds.collect_fold(module_src, FoldKind::Module, line_index);
    }

    folds.collect_folds(&src_map.assign_srcs, FoldKind::ContAssign, line_index);
    folds.collect_folds(&src_map.defparam_srcs, FoldKind::DefParam, line_index);
    folds.collect_folds(&src_map.declaration_srcs, FoldKind::Declaration, line_index);
    folds.collect_folds(&src_map.decl_srcs, FoldKind::Decl, line_index);
    folds.collect_folds(&src_map.opaque_srcs, FoldKind::Opaque, line_index);

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

    folds.collect_docs(&src_map.region_tree, line_index);

    folds.collect_fold(block_src, FoldKind::Block, line_index);
    folds.collect_folds(&src_map.declaration_srcs, FoldKind::Declaration, line_index);
    folds.collect_folds(&src_map.decl_srcs, FoldKind::Decl, line_index);
    folds.collect_folds(&src_map.opaque_srcs, FoldKind::Opaque, line_index);
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
