use hir_def::{
    block::{BlockId, BlockSrc},
    container::{SubroutineParent, SubroutineScope},
    db::HirDefDb,
    module::{
        ModuleId,
        generate::GenerateBlockId,
        instantiation::{
            Instance, InstanceId, InstanceSrc, Instantiation, InstantiationId, InstantiationSrc,
        },
    },
    region_tree::RegionTree,
    source_map::{IsSrc, Lowered, LoweredData, SourceInfo, SourceMap},
    stmt::{Stmt, StmtKind, StmtSrc},
    subroutine::{Subroutine, SubroutineSrc},
};
use la_arena::Arena;
use preproc_expand::{db::PreprocDb, file::HirFileId};
use syntax::{
    SyntaxKind, SyntaxTokenWithParent, SyntaxTrivia,
    has_text_range::HasTextRange,
    token::SyntaxTokenWithParentExt,
    trivia::{TriviaExt, TriviaKindExt},
};
use utils::{
    get::{Get, GetRef},
    line_index::{LineIndex, TextRange},
    text_edit::TextSize,
};
use vfs::FileId;

use crate::db::{line_index_db::LineIndexDb, root_db::RootDb};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldKind {
    Comment,
    Imports,
    Region,
    Module,
    Config,
    Library,
    PortList,
    Decl,
    Declaration,
    ContAssign,
    DefParam,
    Generate,
    Specify,
    Instance,
    Stmt,
    Block,
    Subroutine,
    ArgList,
    Concat,
}

#[derive(Debug)]
pub struct Fold {
    pub range: TextRange,
    pub kind: FoldKind,
}

impl Fold {
    fn new(range: TextRange, kind: FoldKind) -> Self {
        Self { range, kind }
    }

    #[inline]
    fn try_build(range: TextRange, kind: FoldKind, line_index: &LineIndex) -> Option<Self> {
        line_index
            .line_ranges(range)
            .is_some_and(|line_ranges| line_ranges.len() > 1)
            .then(|| Self::new(range, kind))
    }
}

trait FoldCollector {
    fn collect_folds(
        &mut self,
        ranges: impl Iterator<Item = TextRange>,
        kind: FoldKind,
        line_index: &LineIndex,
    );

    fn collect_fold(&mut self, range: TextRange, kind: FoldKind, line_index: &LineIndex);

    fn collect_docs(&mut self, docs: &RegionTree, line_index: &LineIndex);
}

impl FoldCollector for Vec<Fold> {
    #[inline]
    fn collect_folds(
        &mut self,
        ranges: impl Iterator<Item = TextRange>,
        kind: FoldKind,
        line_index: &LineIndex,
    ) {
        self.extend(ranges.filter_map(|range| Fold::try_build(range, kind, line_index)));
    }

    #[inline]
    fn collect_fold(&mut self, range: TextRange, kind: FoldKind, line_index: &LineIndex) {
        if let Some(fold) = Fold::try_build(range, kind, line_index) {
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

/// Collects folds that are purely syntactic: comment groups, multi-line
/// argument lists and concatenations, and runs of consecutive package
/// imports. Everything else is driven by the HIR source maps below.
fn collect_syntax_folds(
    db: &RootDb,
    file_id: HirFileId,
    line_index: &LineIndex,
    folds: &mut Vec<Fold>,
) {
    let tree = db.parse(file_id);
    let Some(root) = tree.root() else {
        return;
    };

    let mut import_ranges = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        match node.kind() {
            SyntaxKind::ARGUMENT_LIST => {
                if let Some(range) = node.text_range() {
                    folds.collect_fold(range, FoldKind::ArgList, line_index);
                }
            }
            SyntaxKind::CONCATENATION_EXPRESSION | SyntaxKind::ASSIGNMENT_PATTERN_EXPRESSION => {
                if let Some(range) = node.text_range() {
                    folds.collect_fold(range, FoldKind::Concat, line_index);
                }
            }
            SyntaxKind::PACKAGE_IMPORT_DECLARATION => {
                if let Some(range) = node.text_range() {
                    import_ranges.push(range);
                }
            }
            _ => {}
        }

        for child in node.children() {
            match child {
                syntax::SyntaxElement::Token(token) => {
                    collect_token_comments(token, line_index, folds)
                }
                syntax::SyntaxElement::Node(node) => stack.push(node),
            }
        }
    }

    // The stack walk visits nodes in reverse sibling order; restore source order.
    import_ranges.sort_by_key(|range| range.start());
    collect_item_groups(folds, import_ranges.into_iter(), FoldKind::Imports, line_index);
}

/// Folds runs of consecutive comments attached to a single token: either a
/// block comment that spans multiple lines, or at least two consecutive line
/// comments. Region markers are excluded, they are folded via the region tree.
fn collect_token_comments(
    token: SyntaxTokenWithParent<'_>,
    line_index: &LineIndex,
    folds: &mut Vec<Fold>,
) {
    let check_lc = |t: &SyntaxTrivia<'_>| {
        t.kind().is_lc() && t.is_region_begin().is_none() && !t.is_region_end()
    };

    let mut trivias = token.trivias_with_range().peekable();
    while let Some((range, t)) = trivias.next() {
        if check_lc(&t) {
            let comment_start = range.start();
            let mut comment_end = None;

            // (1 eol + 1 whitespace (optional) + 1 line comment){>=2}
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
                folds.collect_fold(range, FoldKind::Comment, line_index);
            }
        } else if t.kind().is_bc() {
            folds.collect_fold(range, FoldKind::Comment, line_index);
        }
    }
}

/// Folds consecutive items of the same kind into a single fold: a run of
/// one-line `assign`s, declarations, or package imports collapses into one
/// foldable group. Items are considered consecutive when the next one starts
/// on the line right after the previous one ends (a blank line or another
/// construct breaks the group).
fn collect_item_groups(
    folds: &mut Vec<Fold>,
    ranges: impl IntoIterator<Item = TextRange>,
    kind: FoldKind,
    line_index: &LineIndex,
) {
    let mut group_start: Option<TextSize> = None;
    let mut group_end: Option<TextSize> = None;
    let mut prev_end_line: Option<u32> = None;

    for range in ranges {
        let Some(line_ranges) = line_index.line_ranges(range) else {
            continue;
        };
        let start_line = line_ranges.start as u32;
        let end_line = line_ranges.end as u32 - 1;

        let adjacent = prev_end_line.is_some_and(|prev| start_line == prev + 1);
        if adjacent {
            group_end = Some(range.end());
        } else {
            if let (Some(start), Some(end)) = (group_start, group_end) {
                folds.collect_fold(TextRange::new(start, end), kind, line_index);
            }
            group_start = Some(range.start());
            group_end = Some(range.end());
        }
        prev_end_line = Some(end_line);
    }

    if let (Some(start), Some(end)) = (group_start, group_end) {
        folds.collect_fold(TextRange::new(start, end), kind, line_index);
    }
}

pub(crate) fn folding_ranges(db: &RootDb, file_id: FileId) -> Vec<Fold> {
    let line_index = db.line_index(file_id);
    let line_index = line_index.as_ref();

    let file_id = HirFileId::File(file_id);
    let file = db.hir_file_with_source_map(file_id);
    let src_map = file.source_map();

    let mut folds = Vec::default();

    collect_syntax_folds(db, file_id, line_index, &mut folds);

    folds.collect_docs(&src_map.region_tree, line_index);

    collect_subroutines(
        db,
        &mut folds,
        SubroutineParent::File(file_id),
        &src_map.subroutine_srcs,
        line_index,
    );

    src_map.module_srcs.named_ranges().for_each(|(idx, range, _)| {
        collect_module(db, &mut folds, ModuleId::new(file_id, idx), range, line_index)
    });

    folds.collect_folds(src_map.config_decl_srcs.ranges(), FoldKind::Config, line_index);
    folds.collect_folds(src_map.library_decl_srcs.ranges(), FoldKind::Library, line_index);
    folds.collect_folds(src_map.library_include_srcs.ranges(), FoldKind::Library, line_index);
    folds.collect_folds(src_map.declaration_srcs.ranges(), FoldKind::Declaration, line_index);
    folds.collect_folds(src_map.decl_srcs.ranges(), FoldKind::Decl, line_index);
    collect_item_groups(
        &mut folds,
        src_map.declaration_srcs.ranges(),
        FoldKind::Declaration,
        line_index,
    );
    collect_stmt(
        db,
        &mut folds,
        &file.stmts,
        src_map.stmt_srcs.iter().map(|(id, src)| (id, *src)),
        line_index,
    );

    folds
}

fn collect_module(
    db: &RootDb,
    folds: &mut Vec<Fold>,
    module_id: ModuleId,
    module_range: TextRange,
    line_index: &LineIndex,
) {
    let module = db.module_with_source_map(module_id);
    let src_map = module.source_map();

    folds.collect_docs(&src_map.region_tree, line_index);

    if let Some(port_list_src) = src_map.port_srcs.port_list_src() {
        let port_list_range = SourceInfo::new(*port_list_src).full_range();
        let port_list_fold = Fold::try_build(port_list_range, FoldKind::PortList, line_index);
        let module_body_start = port_list_fold
            .as_ref()
            .and_then(|port_list| {
                let line = line_index.try_line_col(port_list.range.end())?.line + 1;
                line_index.range_for_line(line.min(line_index.lines_len().saturating_sub(1)))
            })
            .unwrap_or(module_range);

        folds.extend(port_list_fold);

        let range = TextRange::new(module_body_start.start(), module_range.end());
        folds.extend(Fold::try_build(range, FoldKind::Module, line_index));
    } else {
        folds.collect_fold(module_range, FoldKind::Module, line_index);
    }

    folds.collect_folds(src_map.assign_srcs.ranges(), FoldKind::ContAssign, line_index);
    folds.collect_folds(src_map.defparam_srcs.ranges(), FoldKind::DefParam, line_index);
    folds.collect_folds(src_map.generate_region_srcs.ranges(), FoldKind::Generate, line_index);
    folds.collect_folds(src_map.specify_block_srcs.ranges(), FoldKind::Specify, line_index);
    folds.collect_folds(src_map.specify_item_srcs.ranges(), FoldKind::Specify, line_index);
    folds.collect_folds(src_map.declaration_srcs.ranges(), FoldKind::Declaration, line_index);
    folds.collect_folds(src_map.decl_srcs.ranges(), FoldKind::Decl, line_index);
    collect_item_groups(folds, src_map.assign_srcs.ranges(), FoldKind::ContAssign, line_index);
    collect_item_groups(
        folds,
        src_map.declaration_srcs.ranges(),
        FoldKind::Declaration,
        line_index,
    );
    collect_instances(folds, &module, &src_map.instance_srcs, line_index);
    collect_subroutines(
        db,
        folds,
        SubroutineParent::Module(module_id),
        &src_map.subroutine_srcs,
        line_index,
    );
    collect_generate_regions(db, folds, module_id, line_index);

    collect_stmt(
        db,
        folds,
        &module.stmts,
        src_map.stmt_srcs.iter().map(|(id, src)| (id, *src)),
        line_index,
    );
}

fn collect_subroutines(
    db: &RootDb,
    folds: &mut Vec<Fold>,
    parent: SubroutineParent,
    srcs: &SourceMap<SubroutineSrc, Subroutine>,
    line_index: &LineIndex,
) {
    for (value, src) in srcs.iter() {
        let scope = SubroutineScope { cont_id: parent, value };
        let subroutine = db.subroutine_with_source_map(scope);
        let src_map = subroutine.source_map();

        folds.collect_docs(&src_map.region_tree, line_index);
        folds.collect_fold(src.range(), FoldKind::Subroutine, line_index);
        folds.collect_folds(src_map.declaration_srcs.ranges(), FoldKind::Declaration, line_index);
        folds.collect_folds(src_map.decl_srcs.ranges(), FoldKind::Decl, line_index);
        collect_item_groups(
            folds,
            src_map.declaration_srcs.ranges(),
            FoldKind::Declaration,
            line_index,
        );
        collect_stmt(
            db,
            folds,
            &subroutine.stmts,
            src_map.stmt_srcs.iter().map(|(id, src)| (id, *src)),
            line_index,
        );
    }
}

fn collect_generate_regions(
    db: &RootDb,
    folds: &mut Vec<Fold>,
    module_id: ModuleId,
    line_index: &LineIndex,
) {
    let module = db.module_with_source_map(module_id);
    let src_map = module.source_map();
    for (region_id, _) in src_map.generate_region_srcs.iter() {
        let region = module.get(region_id);
        for item in &region.items {
            if let hir_def::module::generate::GenerateItem::GenerateBlockId(block_id) = item {
                collect_generate_block(db, folds, *block_id, line_index);
            }
        }
    }
}

fn collect_generate_block(
    db: &RootDb,
    folds: &mut Vec<Fold>,
    block_id: GenerateBlockId,
    line_index: &LineIndex,
) {
    let block = db.generate_block_with_source_map(block_id);
    let src_map = block.source_map();

    folds.collect_docs(&src_map.region_tree, line_index);
    folds.collect_folds(src_map.assign_srcs.ranges(), FoldKind::ContAssign, line_index);
    folds.collect_folds(src_map.defparam_srcs.ranges(), FoldKind::DefParam, line_index);
    folds.collect_folds(src_map.declaration_srcs.ranges(), FoldKind::Declaration, line_index);
    collect_item_groups(folds, src_map.assign_srcs.ranges(), FoldKind::ContAssign, line_index);
    collect_item_groups(
        folds,
        src_map.declaration_srcs.ranges(),
        FoldKind::Declaration,
        line_index,
    );
    collect_instances(folds, &block, &src_map.instance_srcs, line_index);
    collect_subroutines(
        db,
        folds,
        SubroutineParent::GenerateBlock(block_id),
        &src_map.subroutine_srcs,
        line_index,
    );

    for item in &block.items {
        if let hir_def::module::generate::GenerateBlockItem::GenerateBlockId(block_id) = item {
            collect_generate_block(db, folds, *block_id, line_index);
        }
    }
}

fn collect_instances<T, M>(
    folds: &mut Vec<Fold>,
    container: &Lowered<T>,
    instance_srcs: &SourceMap<InstanceSrc, Instance>,
    line_index: &LineIndex,
) where
    T: LoweredData<SourceMap = M>
        + GetRef<InstanceId, Output = Instance>
        + GetRef<InstantiationId, Output = Instantiation>,
    M: Get<InstanceId, Output = Option<InstanceSrc>>
        + Get<InstantiationId, Output = Option<InstantiationSrc>>,
{
    folds.extend(instance_srcs.named_ranges().filter_map(|(instance_id, range, name_range)| {
        let instantiation_id = container.get(instance_id).parent;

        if container.get(instantiation_id).instances.len() > 1 {
            let start = name_range.map_or(range.start(), |range| range.end());
            Fold::try_build(TextRange::new(start, range.end()), FoldKind::Instance, line_index)
        } else {
            Fold::try_build(
                container.source_range(instantiation_id)?,
                FoldKind::Instance,
                line_index,
            )
        }
    }));
}

fn collect_block(
    db: &RootDb,
    folds: &mut Vec<Fold>,
    block_id: BlockId,
    block_range: TextRange,
    line_index: &LineIndex,
) {
    let block = db.block_with_source_map(block_id);
    let src_map = block.source_map();

    folds.collect_docs(&src_map.region_tree, line_index);

    folds.collect_fold(block_range, FoldKind::Block, line_index);
    folds.collect_folds(src_map.declaration_srcs.ranges(), FoldKind::Declaration, line_index);
    folds.collect_folds(src_map.decl_srcs.ranges(), FoldKind::Decl, line_index);
    collect_item_groups(
        folds,
        src_map.declaration_srcs.ranges(),
        FoldKind::Declaration,
        line_index,
    );
    collect_stmt(
        db,
        folds,
        &block.stmts,
        src_map.stmt_srcs.iter().map(|(id, src)| (id, *src)),
        line_index,
    )
}

fn collect_stmt(
    db: &RootDb,
    folds: &mut Vec<Fold>,
    arena: &Arena<Stmt>,
    srcs: impl Iterator<Item = (la_arena::Idx<Stmt>, StmtSrc)>,
    line_index: &LineIndex,
) {
    srcs.for_each(|(stmt_id, stmt_src)| match &arena[stmt_id].kind {
        StmtKind::Block(block_info) => {
            if let Ok(block_src) = BlockSrc::try_from(stmt_src) {
                let range = SourceInfo::new(block_src).full_range();
                collect_block(db, folds, block_info.block_id, range, line_index);
            }
        }
        _ => {
            folds.collect_fold(SourceInfo::new(stmt_src).full_range(), FoldKind::Stmt, line_index);
        }
    });
}
