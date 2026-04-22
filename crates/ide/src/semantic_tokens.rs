use bitflags::bitflags;
use collector::SemaTokenCollectorTree;
use hir::{
    container::{InContainer, InModule},
    db::HirDb,
    file::HirFileId,
    hir_def::{
        Ident,
        block::{BlockId, BlockInfo},
        expr::{Expr, declarator::DeclaratorParent},
        module::{
            ModuleId,
            instantiation::{ParamAssign, PortConn},
        },
        stmt::StmtKind,
    },
    scope::NonAnsiPortEntry,
    semantics::{Semantics, pathres::PathResolution},
    source_map::{IsNamedSrc, IsSrc},
};
use ide_db::root_db::RootDb;
use smol_str::SmolStr;
use syntax::{ast::AstNode, has_text_range::HasTextRange};
use utils::{
    get::{Get, GetRef},
    text_edit::TextRange,
};
use vfs::FileId;

mod collector;
mod port;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SemaTokenConfig {
    pub port: SemaTokenPortConfig,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SemaTokenPortConfig {
    pub clk_rst: bool,
    pub io: bool,
}

impl SemaTokenConfig {
    fn port(&self) -> bool {
        self.port.clk_rst || self.port.io
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SemaToken {
    pub range: TextRange,
    pub tag: SemaTokenTag,
    pub mods: SemaTokenModifier,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SemaTokenTag {
    Port(SemaTokenPort),
    Instance,
    Type,
    None,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SemaTokenPort {
    Clk,
    Rst,
    Others,
}

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct SemaTokenModifier: u32 {
        const DECL = 1 << 0;
        const READ = 1 << 1;
        const WRITE = 1 << 2;
        const REF = 1 << 3;
        const DEF = 1 << 4;
    }
}

struct SemaTokenCollector {
    config: SemaTokenConfig,
    tokens: SemaTokenCollectorTree,
    range: TextRange,
}

impl SemaTokenCollector {
    fn new(config: SemaTokenConfig, range: TextRange) -> Self {
        Self {
            config,
            tokens: SemaTokenCollectorTree::new(SemaToken {
                range,
                tag: SemaTokenTag::None,
                mods: SemaTokenModifier::empty(),
            }),
            range,
        }
    }

    fn finish(self) -> Vec<SemaToken> {
        self.tokens.finish()
    }
}

pub(crate) macro check_range($self:expr, $range:expr) {{
    let range = $range;
    if $self.range.start() >= range.end() {
        continue;
    } else if !$self.range.intersect(range).is_some() {
        break;
    }
}}

impl SemaToken {
    pub fn is_empty(&self) -> bool {
        self.range.is_empty() || (self.tag == SemaTokenTag::None && self.mods.is_empty())
    }
}

pub(crate) fn semantic_tokens(
    db: &RootDb,
    config: SemaTokenConfig,
    file_id: FileId,
    range: Option<TextRange>,
) -> Vec<SemaToken> {
    let sema = Semantics::new(db);
    let file = sema.parse(file_id);
    let range = range.unwrap_or_else(|| file.syntax().text_range().unwrap());
    let file_id = HirFileId(file_id);

    let mut collector = SemaTokenCollector::new(config, range);
    collect_file(&sema, file_id, &mut collector);

    collector.finish()
}

fn collect_file(
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    collector: &mut SemaTokenCollector,
) {
    let (hir_file, file_src_map) = sema.db.hir_file_with_source_map(file_id);

    for (local_module_id, _) in hir_file.modules.iter() {
        let range = file_src_map.get(local_module_id).range();
        check_range!(collector, range);
        collect_module(sema, ModuleId::new(file_id, local_module_id), collector);
    }

    let collect_ident_like =
        |name: &SmolStr, range: TextRange, collector: &mut SemaTokenCollector| {
            let name_in_cont = InContainer::new(file_id.into(), name.clone());
            collect_ident_like(sema, name_in_cont, range, collector);
        };

    for (expr_id, expr) in hir_file.exprs.iter() {
        match expr {
            Expr::Field { .. } => unimplemented!(),
            Expr::Ident(name) => {
                let range = file_src_map.get(expr_id).range();
                check_range!(collector, range);
                collect_ident_like(name, range, collector);
            }
            _ => {}
        }
    }

    for (decl_id, decl) in hir_file.decls.iter() {
        let _: Option<()> = try {
            let name = decl.name.as_ref()?;
            let range = file_src_map.get(decl_id).name_range()?;
            check_range!(collector, range);
            collect_ident_like(name, range, collector);
        };
    }

    for (typedef_id, typedef) in hir_file.typedefs.iter() {
        let _: Option<()> = try {
            let _name = typedef.name.as_ref()?;
            let range = file_src_map.get(typedef_id).name_range()?;
            check_range!(collector, range);
            collector.tokens.add(SemaToken {
                range,
                tag: SemaTokenTag::Type,
                mods: SemaTokenModifier::DECL | SemaTokenModifier::DEF,
            });
        };
    }

    for (stmt_id, stmt) in hir_file.stmts.iter() {
        if let StmtKind::Block(BlockInfo { block_id, .. }) = stmt.kind {
            let range = file_src_map.get(stmt_id).range();
            check_range!(collector, range);
            collect_block(sema, block_id, collector);
        }
    }
}

fn collect_module(
    sema: &Semantics<'_, RootDb>,
    module_id: ModuleId,
    collector: &mut SemaTokenCollector,
) {
    let db = sema.db;
    let (module, module_src_map) = db.module_with_source_map(module_id);
    let (module, module_src_map) = (module.as_ref(), module_src_map.as_ref());
    port::collect_port(sema, module_id, collector);

    let collect_ident_like =
        |name: &SmolStr, range: TextRange, collector: &mut SemaTokenCollector| {
            let name_in_cont = InContainer::new(module_id.into(), name.clone());
            collect_ident_like(sema, name_in_cont, range, collector);
        };

    for (instance_id, _) in module.instances.iter() {
        if let Some(range) = module_src_map.get(instance_id).name_range() {
            check_range!(collector, range);
            let sema_token =
                SemaToken { range, tag: SemaTokenTag::Instance, mods: SemaTokenModifier::empty() };
            collector.tokens.add(sema_token);
        };
    }

    for (param_assign_id, param_assign) in module.inst_param_assigns.iter() {
        let Some(range) = module_src_map.get(param_assign_id).name_range() else {
            continue;
        };
        check_range!(collector, range);

        match param_assign {
            ParamAssign::Named(Some(name), _) => {
                check_range!(collector, range);
                collect_ident_like(name, range, collector);
            }
            ParamAssign::Named(..) | ParamAssign::Ordered(_) => {}
        }
    }

    for (conn_id, conn) in module.inst_port_conns.iter() {
        match conn {
            PortConn::Named(Some(name), _) => {
                let range = module_src_map.get(conn_id).range();
                check_range!(collector, range);
                collect_ident_like(name, range, collector);
            }
            PortConn::Named(..) | PortConn::Empty | PortConn::Ordered(_) | PortConn::Wildcard => {}
        }
    }

    for (expr_id, expr) in module.exprs.iter() {
        match expr {
            Expr::Field { .. } => unimplemented!(),
            Expr::Ident(name) => {
                let range = module_src_map.get(expr_id).range();
                check_range!(collector, range);
                collect_ident_like(name, range, collector);
            }
            _ => {}
        }
    }

    for (decl_id, decl) in module.decls.iter() {
        let _: Option<()> = try {
            let name = decl.name.as_ref()?;
            let range = module_src_map.get(decl_id).name_range()?;
            check_range!(collector, range);
            collect_ident_like(name, range, collector);
        };
    }

    for (typedef_id, typedef) in module.typedefs.iter() {
        let _: Option<()> = try {
            let _name = typedef.name.as_ref()?;
            let range = module_src_map.get(typedef_id).name_range()?;
            check_range!(collector, range);
            collector.tokens.add(SemaToken {
                range,
                tag: SemaTokenTag::Type,
                mods: SemaTokenModifier::DECL | SemaTokenModifier::DEF,
            });
        };
    }

    for (stmt_id, stmt) in module.stmts.iter() {
        if let StmtKind::Block(BlockInfo { block_id, .. }) = stmt.kind {
            let range = module_src_map.get(stmt_id).range();
            check_range!(collector, range);
            collect_block(sema, block_id, collector);
        }
    }
}

fn collect_block(
    sema: &Semantics<'_, RootDb>,
    block_id: BlockId,
    collector: &mut SemaTokenCollector,
) {
    let db = sema.db;
    let (block, block_src_map) = db.block_with_source_map(block_id);
    let (block, block_src_map) = (block.as_ref(), block_src_map.as_ref());

    let collect_ident_like =
        |name: &SmolStr, range: TextRange, collector: &mut SemaTokenCollector| {
            let name_in_cont = InContainer::new(block_id.into(), name.clone());
            collect_ident_like(sema, name_in_cont, range, collector);
        };

    for (expr_id, expr) in block.exprs.iter() {
        match expr {
            Expr::Field { .. } => unimplemented!(),
            Expr::Ident(name) => {
                let range = block_src_map.get(expr_id).range();
                check_range!(collector, range);
                collect_ident_like(name, range, collector);
            }
            _ => {}
        }
    }

    for (decl_id, decl) in block.decls.iter() {
        let _: Option<()> = try {
            let name = decl.name.as_ref()?;
            let range = block_src_map.get(decl_id).name_range()?;
            check_range!(collector, range);
            collect_ident_like(name, range, collector);
        };
    }

    for (typedef_id, typedef) in block.typedefs.iter() {
        let _: Option<()> = try {
            let _name = typedef.name.as_ref()?;
            let range = block_src_map.get(typedef_id).name_range()?;
            check_range!(collector, range);
            collector.tokens.add(SemaToken {
                range,
                tag: SemaTokenTag::Type,
                mods: SemaTokenModifier::DECL | SemaTokenModifier::DEF,
            });
        };
    }

    for (stmt_id, stmt) in block.stmts.iter() {
        if let StmtKind::Block(BlockInfo { block_id, .. }) = stmt.kind {
            let range = block_src_map.get(stmt_id).range();
            check_range!(collector, range);
            collect_block(sema, block_id, collector);
        }
    }
}

fn collect_ident_like(
    sema: &Semantics<'_, RootDb>,
    in_cont: InContainer<Ident>,
    range: TextRange,
    collector: &mut SemaTokenCollector,
) -> Option<()> {
    let db = sema.db;
    let res = sema.name_to_def(in_cont)?;

    match res {
        PathResolution::NonAnsiPort { label, port_decl, data_decl, module } => {
            let module = db.module(module);
            let name = module.get(port_decl?).name.as_ref()?;
            let entry = NonAnsiPortEntry { label, port_decl, data_decl };
            let (dir, ty) = port::resolve_non_ansi_port(module.as_ref(), &entry.into())?;
            port::add_port_token(db, name, dir, ty, range, collector);
        }
        PathResolution::AnsiPort(InModule { value: decl_id, module_id }) => {
            let module = db.module(module_id);
            let name = module.get(decl_id).name.as_ref()?;

            let DeclaratorParent::PortDeclId(port_declaration_id) = module.get(decl_id).parent
            else {
                unreachable!();
            };
            let port_decl = module.get(port_declaration_id);
            let header = &port_decl.header;
            let (dir, ty) = (header.dir(), header.ty());
            port::add_port_token(db, name, dir, ty, range, collector);
        }
        PathResolution::Instance(_) => {
            let sema_token =
                SemaToken { range, tag: SemaTokenTag::Instance, mods: SemaTokenModifier::empty() };
            collector.tokens.add(sema_token);
        }
        PathResolution::Typedef(_) => {
            collector.tokens.add(SemaToken {
                range,
                tag: SemaTokenTag::Type,
                mods: SemaTokenModifier::REF,
            });
        }
        _ => {}
    }

    Some(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use base_db::{change::Change, source_root::SourceRoot};
    use insta::assert_debug_snapshot;
    use triomphe::Arc;
    use utils::{lines::LineEnding, text_edit::TextRange};
    use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

    use super::*;
    use crate::{analysis_host::AnalysisHost, test_utils::normalize_fixture_text};

    fn setup(text: &str) -> (AnalysisHost, FileId) {
        let text = normalize_fixture_text(text);
        let file_id = FileId(0);
        let path = VfsPath::new_virtual_path("/test.v".to_string());

        let mut file_set = FileSet::default();
        file_set.insert(file_id, path);
        let root = SourceRoot::new_local(file_set);

        let mut change = Change::new();
        change.set_roots(vec![root]);
        change.add_changed_file(ChangedFile {
            file_id,
            change_kind: ChangeKind::Create(Arc::from(text.as_str()), LineEnding::Unix),
        });

        let mut host = AnalysisHost::default();
        host.apply_change(change);
        (host, file_id)
    }

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/semantic_tokens/fixtures")
    }

    #[test]
    fn semantic_token_fixtures() {
        let dir = fixtures_dir();
        let mut fixtures: Vec<(String, PathBuf)> = std::fs::read_dir(&dir)
            .unwrap_or_else(|err| panic!("failed to read fixtures dir {dir:?}: {err}"))
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.extension()? != "v" {
                    return None;
                }
                let name = path.file_stem()?.to_string_lossy().to_string();
                Some((name, path))
            })
            .collect();

        fixtures.sort_by(|a, b| a.0.cmp(&b.0));
        assert!(!fixtures.is_empty(), "no fixtures found in {dir:?}");

        for (name, path) in fixtures {
            let text =
                std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {path:?}: {err}"));
            let text = normalize_fixture_text(&text);
            let (host, file_id) = setup(&text);
            let tokens = host
                .make_analysis()
                .semantic_tokens(
                    file_id,
                    SemaTokenConfig { port: SemaTokenPortConfig { clk_rst: false, io: false } },
                    Some(TextRange::up_to(utils::text_edit::TextSize::of(text.as_str()))),
                )
                .unwrap();
            assert_debug_snapshot!(name, tokens);
        }
    }
}
