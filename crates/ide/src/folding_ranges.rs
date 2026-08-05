use hir_def::{
    block::{BlockId, BlockSrc},
    container::{SubroutineParent, SubroutineScope},
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
use preproc_expand::file::HirFileId;
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
    collect_item_groups(folds, import_ranges, FoldKind::Imports, line_index);
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
    let mut group: Option<(TextSize, TextSize, usize)> = None; // (start, end, len)
    let mut prev_end_line: Option<u32> = None;

    let flush = |folds: &mut Vec<Fold>, group: &mut Option<(TextSize, TextSize, usize)>| {
        if let Some((start, end, len)) = group.take()
            && len > 1
        {
            folds.collect_fold(TextRange::new(start, end), kind, line_index);
        }
    };

    for range in ranges {
        let Some(line_ranges) = line_index.line_ranges(range) else {
            continue;
        };
        let start_line = line_ranges.start as u32;
        let end_line = line_ranges.end as u32 - 1;

        let adjacent = prev_end_line.is_some_and(|prev| start_line == prev + 1);
        if adjacent {
            let (start, _, len) = group.unwrap();
            group = Some((start, range.end(), len + 1));
        } else {
            flush(folds, &mut group);
            group = Some((range.start(), range.end(), 1));
        }
        prev_end_line = Some(end_line);
    }
    flush(folds, &mut group);
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
        let scope = SubroutineScope { cont_id: parent.clone(), value };
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
                collect_generate_block(db, folds, block_id.clone(), line_index);
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
    let block = db.generate_block_with_source_map(block_id.clone());
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
        SubroutineParent::GenerateBlock(block_id.clone()),
        &src_map.subroutine_srcs,
        line_index,
    );

    for item in &block.items {
        if let hir_def::module::generate::GenerateBlockItem::GenerateBlockId(block_id) = item {
            collect_generate_block(db, folds, block_id.clone(), line_index);
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
                collect_block(db, folds, block_info.block_id.clone(), range, line_index);
            }
        }
        _ => {
            folds.collect_fold(SourceInfo::new(stmt_src).full_range(), FoldKind::Stmt, line_index);
        }
    });
}

#[cfg(test)]
mod tests {
    use base_db::{change::Change, source_root::SourceRoot};
    use utils::line_index::{TextRange, TextSize};
    use vfs::{ChangedFile, FileId, FileSet, VfsPath};

    use super::{Fold, FoldKind, folding_ranges};
    use crate::db::{line_index_db::LineIndexDb, root_db::RootDb};

    fn db_with_file(text: &str) -> (RootDb, FileId) {
        let file_id = FileId::from_raw(0);
        let path = VfsPath::new_virtual_path("/test.sv".to_owned());

        let mut file_set = FileSet::default();
        file_set.insert(file_id, path);
        let root = SourceRoot::new_local(file_set);

        let mut change = Change::new();
        change.set_roots(vec![root]);
        change.add_changed_file(ChangedFile::create(file_id, text));

        let mut db = RootDb::new(None);
        change.apply(&mut db);
        (db, file_id)
    }

    /// Extracts `<fold kind>...</fold>` tags from a fixture, returning the
    /// cleaned text and the expected fold ranges (in cleaned-text offsets).
    fn extract_fold_tags(text: &str) -> (String, Vec<(TextRange, FoldKind)>) {
        let mut clean = String::new();
        let mut expected = Vec::new();
        let mut stack: Vec<(usize, FoldKind)> = Vec::new();
        let mut rest = text;

        loop {
            let open = rest.find("<fold ");
            let close = rest.find("</fold>");
            match (open, close) {
                (Some(start), Some(close)) if start < close => {
                    let name_end = rest[start..].find('>').map(|i| start + i).unwrap();
                    let kind = parse_fold_kind(&rest[start + 6..name_end]);
                    clean.push_str(&rest[..start]);
                    stack.push((clean.len(), kind));
                    rest = &rest[name_end + 1..];
                }
                (Some(start), None) => {
                    let name_end = rest[start..].find('>').map(|i| start + i).unwrap();
                    let kind = parse_fold_kind(&rest[start + 6..name_end]);
                    clean.push_str(&rest[..start]);
                    stack.push((clean.len(), kind));
                    rest = &rest[name_end + 1..];
                }
                (_, Some(rel)) => {
                    clean.push_str(&rest[..rel]);
                    let (range_start, kind) = stack.pop().expect("unbalanced </fold>");
                    expected.push((
                        TextRange::new(
                            TextSize::from(range_start as u32),
                            TextSize::from(clean.len() as u32),
                        ),
                        kind,
                    ));
                    rest = &rest[rel + 7..];
                }
                (None, None) => {
                    clean.push_str(rest);
                    break;
                }
            }
        }
        assert!(stack.is_empty(), "unbalanced <fold> tags");
        (clean, expected)
    }

    fn parse_fold_kind(name: &str) -> FoldKind {
        match name {
            "comment" => FoldKind::Comment,
            "imports" => FoldKind::Imports,
            "region" => FoldKind::Region,
            "module" => FoldKind::Module,
            "config" => FoldKind::Config,
            "library" => FoldKind::Library,
            "portlist" => FoldKind::PortList,
            "decl" => FoldKind::Decl,
            "declaration" => FoldKind::Declaration,
            "contassign" => FoldKind::ContAssign,
            "defparam" => FoldKind::DefParam,
            "generate" => FoldKind::Generate,
            "specify" => FoldKind::Specify,
            "instance" => FoldKind::Instance,
            "stmt" => FoldKind::Stmt,
            "block" => FoldKind::Block,
            "subroutine" => FoldKind::Subroutine,
            "arglist" => FoldKind::ArgList,
            "concat" => FoldKind::Concat,
            other => panic!("unknown fold kind {other:?}"),
        }
    }

    fn check_folds(fixture: &str) {
        let (text, mut expected) = extract_fold_tags(fixture);
        let (db, file_id) = db_with_file(&text);
        let mut folds = folding_ranges(&db, file_id);

        // Equal-range folds (e.g. a pseudo region and its declaration group)
        // may arrive in any order, so tie-break by kind.
        let order =
            |fold: &Fold| (fold.range.start(), fold.range.end(), format!("{:?}", fold.kind));
        folds.sort_by_key(order);
        expected.sort_by_key(|(range, kind)| (range.start(), range.end(), format!("{kind:?}")));

        if folds.len() != expected.len() {
            let mut report = String::new();
            for fold in &folds {
                report.push_str(&format!("  {:?} {:?}\n", fold.kind, fold.range));
            }
            panic!(
                "fold count mismatch: got {} folds, expected {}\n{report}",
                folds.len(),
                expected.len()
            );
        }
        for (fold, (range, kind)) in folds.iter().zip(expected) {
            assert_eq!(fold.range, range, "range mismatch");
            assert_eq!(fold.kind, kind, "kind mismatch for {range:?}");
        }
    }

    #[test]
    fn fold_comments() {
        check_folds(
            r#"<fold comment>// one
// two</fold>

// standalone

<fold comment>/* block
 * comment */</fold>
"#,
        );
    }

    #[test]
    fn fold_comments_do_not_cross_blank_lines() {
        check_folds(
            r#"// one

<fold comment>// two
// three</fold>

// four
module m; endmodule
"#,
        );
    }

    #[test]
    fn fold_regions() {
        check_folds(
            r#"<fold region>// region: file
// body
// endregion</fold>

<fold module>module m;
  <fold region>// region: inner
  logic x;
  // endregion</fold>
endmodule</fold>
"#,
        );
    }

    #[test]
    fn fold_region_markers_are_not_comment_folds() {
        check_folds(
            r#"<fold region>// region: keep
// marker lines are folded as a region, not as a comment run
// endregion</fold>
"#,
        );
    }

    #[test]
    fn fold_module_with_port_list() {
        check_folds(
            r#"module m <fold portlist>(
  input logic a,
  output logic b
)</fold>;
<fold module><fold declaration>logic x;
logic y;</fold>
  always_comb <fold block>begin
    x = a;
  end</fold>
endmodule</fold>
"#,
        );
    }

    #[test]
    fn fold_module_without_port_list_fold() {
        check_folds(
            r#"<fold module>module m(a, b);
  logic x;
endmodule</fold>
"#,
        );
    }

    #[test]
    fn fold_subroutines() {
        check_folds(
            r#"<fold module>module m;
<fold subroutine>function logic f;
  <fold block>begin
    x = a;
  end</fold>
endfunction</fold>

<fold subroutine>task t;
  x = a;
endtask</fold>
endmodule</fold>
"#,
        );
    }

    #[test]
    fn fold_file_level_subroutine() {
        check_folds(
            r#"<fold subroutine>function logic file_f;
  return a;
endfunction</fold>

module m; endmodule
"#,
        );
    }

    #[test]
    fn fold_instances() {
        check_folds(
            r#"<fold module>module m;
<fold instance>u_mod u0 (
  .a(a),
  .b(b)
);</fold>
endmodule</fold>
"#,
        );
    }

    #[test]
    fn fold_multi_instance_folds_from_name_end() {
        check_folds(
            r#"<fold module>module m;
u_mod u0<fold instance> (
  .a(a)
)</fold>,
u1<fold instance> (
  .b(b)
)</fold>;
endmodule</fold>
"#,
        );
    }

    #[test]
    fn fold_generate_block() {
        check_folds(
            r#"<fold module>module m;
<fold generate>generate
  if (COND) begin : genblk
    <fold instance>u_mod u1 (
      .a(a)
    );</fold>
  end
endgenerate</fold>
endmodule</fold>
"#,
        );
    }

    #[test]
    fn fold_import_groups() {
        check_folds(
            r#"<fold imports>import pkg_a::*;
import pkg_b::*;</fold>

<fold module>module m;
  <fold imports>import pkg_c::*;
  import pkg_d::*;</fold>
endmodule</fold>
"#,
        );
    }

    #[test]
    fn fold_assign_groups() {
        check_folds(
            r#"<fold module>module m;
<fold contassign>assign a = x;
assign b = y;</fold>

assign c = z;
endmodule</fold>
"#,
        );
    }

    #[test]
    fn fold_declaration_group_breaks_on_other_items() {
        check_folds(
            r#"<fold module>module m;
<fold declaration>logic a;
logic b;</fold>
assign x = y;
<fold declaration>logic c;
logic d;</fold>
endmodule</fold>
"#,
        );
    }

    #[test]
    fn fold_argument_lists_and_concatenations() {
        check_folds(
            r#"<fold module>module m;
  always_comb <fold block>begin
    <fold stmt>x = foo<fold arglist>(a,
      b,
      c)</fold>;</fold>
    <fold stmt>y = <fold concat>{a,
      b,
      c}</fold>;</fold>
  end</fold>
endmodule</fold>
"#,
        );
    }

    #[test]
    fn fold_pseudo_region() {
        check_folds(
            r#"<fold module>module m;
// grouped declarations
<fold region><fold declaration>logic a;
logic b;</fold></fold>
endmodule</fold>
"#,
        );
    }

    #[test]
    fn fold_single_item_group_does_not_fold_twice() {
        // A lone multi-line declaration folds once: the item fold, not a
        // single-element group on top of it.
        check_folds(
            r#"<fold module>module m;
<fold declaration>logic a,
  b;</fold>
endmodule</fold>
"#,
        );
    }

    #[test]
    fn fold_config() {
        check_folds(
            r#"<fold config>config cfg;
  design top;
endconfig</fold>
"#,
        );
    }
}
