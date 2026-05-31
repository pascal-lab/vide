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
    source_map::{IsNamedSrc, IsSrc, ToAstNode},
};
use smol_str::SmolStr;
use syntax::{ast, has_text_range::HasTextRange};
use utils::{
    get::{Get, GetRef},
    text_edit::TextRange,
};
use vfs::FileId;

use crate::{
    db::root_db::RootDb,
    module_resolution::{resolve_named_param_assignment, resolve_named_port_connection},
};

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
    let parsed_file = sema.parse_file(file_id);
    let Some(root) = parsed_file.root() else {
        return Vec::new();
    };
    let file_id = HirFileId(file_id);
    let range = match range {
        Some(range) => range,
        None => {
            let Some(root_range) = root.text_range() else {
                return Vec::new();
            };
            root_range
        }
    };

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
        let Some(range) = file_src_map.get(local_module_id).map(|src| src.range()) else {
            continue;
        };
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
            Expr::Field { .. } => {}
            Expr::Ident(name) => {
                let Some(range) = file_src_map.get(expr_id).map(|src| src.range()) else {
                    continue;
                };
                check_range!(collector, range);
                collect_ident_like(name, range, collector);
            }
            _ => {}
        }
    }

    for (decl_id, decl) in hir_file.decls.iter() {
        let _: Option<()> = try {
            let name = decl.name.as_ref()?;
            let range = file_src_map.get(decl_id)?.name_range()?;
            check_range!(collector, range);
            collect_ident_like(name, range, collector);
        };
    }

    for (typedef_id, typedef) in hir_file.typedefs.iter() {
        let _: Option<()> = try {
            let _name = typedef.name.as_ref()?;
            let range = file_src_map.get(typedef_id)?.name_range()?;
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
            let Some(range) = file_src_map.get(stmt_id).map(|src| src.range()) else {
                continue;
            };
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
        if let Some(range) = module_src_map.get(instance_id).and_then(|src| src.name_range()) {
            check_range!(collector, range);
            let sema_token =
                SemaToken { range, tag: SemaTokenTag::Instance, mods: SemaTokenModifier::empty() };
            collector.tokens.add(sema_token);
        };
    }

    collect_named_param_assignments(sema, module_id, collector);
    collect_named_port_connections(sema, module_id, collector);

    for (expr_id, expr) in module.exprs.iter() {
        match expr {
            Expr::Field { .. } => {}
            Expr::Ident(name) => {
                let Some(range) = module_src_map.get(expr_id).map(|src| src.range()) else {
                    continue;
                };
                check_range!(collector, range);
                collect_ident_like(name, range, collector);
            }
            _ => {}
        }
    }

    for (decl_id, decl) in module.decls.iter() {
        let _: Option<()> = try {
            let name = decl.name.as_ref()?;
            let range = module_src_map.get(decl_id)?.name_range()?;
            check_range!(collector, range);
            collect_ident_like(name, range, collector);
        };
    }

    for (typedef_id, typedef) in module.typedefs.iter() {
        let _: Option<()> = try {
            let _name = typedef.name.as_ref()?;
            let range = module_src_map.get(typedef_id)?.name_range()?;
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
            let Some(range) = module_src_map.get(stmt_id).map(|src| src.range()) else {
                continue;
            };
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
            Expr::Field { .. } => {}
            Expr::Ident(name) => {
                let Some(range) = block_src_map.get(expr_id).map(|src| src.range()) else {
                    continue;
                };
                check_range!(collector, range);
                collect_ident_like(name, range, collector);
            }
            _ => {}
        }
    }

    for (decl_id, decl) in block.decls.iter() {
        let _: Option<()> = try {
            let name = decl.name.as_ref()?;
            let range = block_src_map.get(decl_id)?.name_range()?;
            check_range!(collector, range);
            collect_ident_like(name, range, collector);
        };
    }

    for (typedef_id, typedef) in block.typedefs.iter() {
        let _: Option<()> = try {
            let _name = typedef.name.as_ref()?;
            let range = block_src_map.get(typedef_id)?.name_range()?;
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
            let Some(range) = block_src_map.get(stmt_id).map(|src| src.range()) else {
                continue;
            };
            check_range!(collector, range);
            collect_block(sema, block_id, collector);
        }
    }
}

fn collect_named_port_connections(
    sema: &Semantics<'_, RootDb>,
    module_id: ModuleId,
    collector: &mut SemaTokenCollector,
) {
    let db = sema.db;
    let (module, module_src_map) = db.module_with_source_map(module_id);
    let (module, module_src_map) = (module.as_ref(), module_src_map.as_ref());
    let tree = db.parse(module_id.file_id);

    for (conn_id, conn) in module.inst_port_conns.iter() {
        let PortConn::Named(Some(_), _) = conn else {
            continue;
        };
        let Some(src) = module_src_map.get(conn_id) else {
            continue;
        };
        let Some(range) = src.name_range() else {
            continue;
        };
        check_range!(collector, range);

        let Some(named) =
            src.to_node(&tree).and_then(ast::PortConnection::as_named_port_connection)
        else {
            continue;
        };
        if let Some(res) = resolve_named_port_connection(db, module_id.file_id.file_id(), named) {
            collect_resolved_path(sema, res, range, collector);
        }
    }
}

fn collect_named_param_assignments(
    sema: &Semantics<'_, RootDb>,
    module_id: ModuleId,
    collector: &mut SemaTokenCollector,
) {
    let db = sema.db;
    let (module, module_src_map) = db.module_with_source_map(module_id);
    let (module, module_src_map) = (module.as_ref(), module_src_map.as_ref());
    let tree = db.parse(module_id.file_id);

    for (assign_id, assign) in module.inst_param_assigns.iter() {
        let ParamAssign::Named(Some(_), _) = assign else {
            continue;
        };
        let Some(src) = module_src_map.get(assign_id) else {
            continue;
        };
        let Some(range) = src.name_range() else {
            continue;
        };
        check_range!(collector, range);

        let Some(named) =
            src.to_node(&tree).and_then(ast::ParamAssignment::as_named_param_assignment)
        else {
            continue;
        };
        if let Some(res) = resolve_named_param_assignment(db, module_id.file_id.file_id(), named) {
            collect_resolved_path(sema, res, range, collector);
        }
    }
}

fn collect_ident_like(
    sema: &Semantics<'_, RootDb>,
    in_cont: InContainer<Ident>,
    range: TextRange,
    collector: &mut SemaTokenCollector,
) -> Option<()> {
    let res = sema.name_to_def(in_cont)?;
    collect_resolved_path(sema, res, range, collector)
}

fn collect_resolved_path(
    sema: &Semantics<'_, RootDb>,
    res: PathResolution,
    range: TextRange,
    collector: &mut SemaTokenCollector,
) -> Option<()> {
    let db = sema.db;

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
                return None;
            };
            let port_decl = module.get(port_declaration_id);
            let header = &port_decl.header;
            let (dir, ty) = (Some(header.dir()), header.ty());
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

    use hir::base_db::{change::Change, source_root::SourceRoot};
    use insta::assert_debug_snapshot;
    use triomphe::Arc;
    use utils::{
        lines::LineEnding,
        text_edit::{TextRange, TextSize},
    };
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

    #[test]
    fn named_port_connection_labels_use_target_module_ports() {
        let text = r#"
module darksocv
(
    input        UART_RXD,
    output [31:0] LED,
    input  [31:0] IPORT,
    output [31:0] OPORT,
    output [3:0]  DEBUG
);
    wire [31:0] iport;
    wire [31:0] oport;

    darkio io0 (
        .RXD    (UART_RXD),
        .TXD    (UART_TXD),
        .LED    (LED),
        .IPORT  (iport),
        .OPORT  (oport),
        .DEBUG  (IODEBUG)
    );
endmodule

module darkio
(
    input         RXD,
    output        TXD,
    output [31:0] LED,
    input  [31:0] IPORT,
    output [31:0] OPORT,
    output  [3:0] DEBUG
);
endmodule
"#;
        let (host, file_id) = setup(text);
        let tokens = host
            .make_analysis()
            .semantic_tokens(
                file_id,
                SemaTokenConfig { port: SemaTokenPortConfig { clk_rst: true, io: true } },
                Some(TextRange::up_to(TextSize::of(text))),
            )
            .unwrap();

        let token = |needle: &str| {
            let start = text.find(needle).expect("connection label should exist") + 1;
            let end = start + needle.len() - 1;
            let range = TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32));
            tokens
                .iter()
                .find(|token| !token.is_empty() && token.range == range)
                .copied()
                .unwrap_or_else(|| panic!("expected token at {range:?} for {needle}"))
        };

        assert_eq!(
            (token(".RXD").tag, token(".RXD").mods),
            (SemaTokenTag::Port(SemaTokenPort::Others), SemaTokenModifier::READ)
        );
        assert_eq!(
            (token(".TXD").tag, token(".TXD").mods),
            (SemaTokenTag::Port(SemaTokenPort::Others), SemaTokenModifier::WRITE)
        );
        assert_eq!(
            (token(".LED").tag, token(".LED").mods),
            (SemaTokenTag::Port(SemaTokenPort::Others), SemaTokenModifier::WRITE)
        );
        assert_eq!(
            (token(".IPORT").tag, token(".IPORT").mods),
            (SemaTokenTag::Port(SemaTokenPort::Others), SemaTokenModifier::READ)
        );
        assert_eq!(
            (token(".OPORT").tag, token(".OPORT").mods),
            (SemaTokenTag::Port(SemaTokenPort::Others), SemaTokenModifier::WRITE)
        );
        assert_eq!(
            (token(".DEBUG").tag, token(".DEBUG").mods),
            (SemaTokenTag::Port(SemaTokenPort::Others), SemaTokenModifier::WRITE)
        );
    }

    #[test]
    fn named_port_connection_token_uses_name_range() {
        let text = "\
module child(output logic instr_req_o);
endmodule

module top(output logic instr_req_o);
child u_child (
    .instr_req_o (instr_req_o),
);
endmodule
";
        let (host, file_id) = setup(text);
        let tokens = host
            .make_analysis()
            .semantic_tokens(
                file_id,
                SemaTokenConfig { port: SemaTokenPortConfig { clk_rst: false, io: true } },
                Some(TextRange::up_to(TextSize::of(text))),
            )
            .unwrap();

        let named_port_start = text.find(".instr_req_o").unwrap() + 1;
        let named_port_range = TextRange::new(
            TextSize::from(named_port_start as u32),
            TextSize::from((named_port_start + "instr_req_o".len()) as u32),
        );
        let expr_start = text.find("(instr_req_o)").unwrap() + 1;
        let expr_range = TextRange::new(
            TextSize::from(expr_start as u32),
            TextSize::from((expr_start + "instr_req_o".len()) as u32),
        );

        assert!(tokens.iter().any(|token| token.range == named_port_range));
        assert!(tokens.iter().any(|token| token.range == expr_range));
        assert!(
            tokens
                .iter()
                .all(|token| token.range
                    != TextRange::new(named_port_range.start(), expr_range.end())),
            "named port connection must not produce a token spanning the whole connection"
        );
    }
}
