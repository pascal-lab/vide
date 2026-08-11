use bitflags::bitflags;
use collector::SemaTokenCollectorTree;
use hir_def::{
    Ident,
    body::BodyItem,
    container::{InFile, OwnerRef},
    def_id::DefId,
    expr::{
        Expr, ExprId,
        data_ty::{DataTy, TypeRef},
        declarator::DeclaratorParent,
    },
    has_source::HasSource,
    module::instantiation::{ParamAssign, ParamAssignId, PortConn, PortConnId},
    owner::OwnerId,
    pathres::{NameRef, RefKind, resolve_path},
    source_map::AstLookup,
    symbol::{DefKind, NameContext, Resolution},
};
use hir_semantics::semantics::Semantics;
use preproc_expand::{file::HirFileId, preproc::macro_references_in_range};
use rustc_hash::FxHashSet;
use smol_str::SmolStr;
use syntax::{
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
};
use tracing::warn;
use utils::text_edit::TextRange;
use vfs::FileId;

use crate::{
    db::root_db::RootDb,
    module_resolution::{
        resolve_named_param_assignment, resolve_named_port_connection, resolve_port_metadata,
    },
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
    Macro,
    Type,
    TomlKey,
    TomlString,
    TomlNumber,
    TomlBoolean,
    TomlValue,
    TomlComment,
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
    let _span = tracing::debug_span!("ide.semantic_tokens", ?file_id, ?range).entered();
    if db.file_kind(file_id).is_project_manifest() {
        return crate::manifest::semantic_tokens(db, file_id, range);
    }
    let sema = Semantics::new(db);
    let parsed_file = sema.parse_file(file_id);
    let Some(root) = parsed_file.root() else {
        return Vec::new();
    };
    let file_id = HirFileId::File(file_id);
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
    collect_preproc_macro_references(db, file_id.expect_file(), range, &mut collector);

    collector.finish()
}

fn collect_preproc_macro_references(
    db: &RootDb,
    file_id: FileId,
    range: TextRange,
    collector: &mut SemaTokenCollector,
) {
    let references = match macro_references_in_range(db, file_id, range) {
        Ok(references) => references,
        Err(error) => {
            warn!(?file_id, ?range, ?error, "semantic macro tokens unavailable");
            return;
        }
    };

    for reference in references {
        if reference.range.intersect(collector.range).is_none() {
            continue;
        }
        collector.tokens.add(SemaToken {
            range: reference.range,
            tag: SemaTokenTag::Macro,
            mods: SemaTokenModifier::REF,
        });
    }
}

/// Collects the ident-like tokens shared by every HIR container (file, module,
/// block, generate block, subroutine): named-data-type expressions, identifier
/// expressions, declaration names, typedef names, and nested blocks.
macro_rules! collect_container_body {
    ($sema:expr, $cont_id:expr, $tree:expr, $collector:expr, $lowered:expr) => {{
        let sema = $sema;
        let db = sema.db;
        let cont_id: OwnerId = $cont_id;
        let tree = $tree;
        let collector = $collector;
        let lowered = $lowered;

        let collect_ident_like =
            |name: &SmolStr, range: TextRange, collector: &mut SemaTokenCollector| {
                let name_in_cont = OwnerRef::new(cont_id.clone(), name.clone());
                collect_ident_like(sema, name_in_cont, range, collector);
            };

        // Call callees search every scope to its end (IEEE 1800-2017 26.3);
        // ordinary references are resolved at their source position.
        let mut call_callees = FxHashSet::default();
        for (_, expr) in lowered.data_ref().exprs.iter() {
            if let Expr::Call { callee, .. } = expr {
                call_callees.insert(*callee);
            }
        }

        for (_, declaration) in lowered.data_ref().declarations.iter() {
            let DataTy::Named(type_ref) = declaration.ty() else {
                continue;
            };
            let Some(range) = db
                .source_projection(cont_id.file(db))
                .origin(type_ref.source())
                .and_then(|origin| origin.focus_or_full_range())
            else {
                continue;
            };
            check_range!(collector, range);
            collect_type_ref_like(sema, cont_id.clone(), &type_ref, range, collector);
        }
        for (_, typedef) in lowered.data_ref().typedefs.iter() {
            let Some(DataTy::Named(type_ref)) = typedef.ty.clone() else {
                continue;
            };
            let Some(range) = db
                .source_projection(cont_id.file(db))
                .origin(type_ref.source())
                .and_then(|origin| origin.focus_or_full_range())
            else {
                continue;
            };
            check_range!(collector, range);
            collect_type_ref_like(sema, cont_id.clone(), &type_ref, range, collector);
        }

        for (expr_id, expr) in lowered.data_ref().exprs.iter() {
            match expr {
                Expr::Field { .. } => {
                    let _: Option<()> = try {
                        let expr = lowered.ast(db, expr_id, tree)?;
                        collect_field_like(sema, cont_id.clone(), expr_id, expr, collector)?;
                    };
                }
                Expr::Ident(name) => {
                    let Some(range) = lowered.source_range(db, expr_id) else {
                        continue;
                    };
                    check_range!(collector, range);
                    let Some(source) = lowered.source_map().expr_srcs.hir_to_src(expr_id) else {
                        continue;
                    };
                    let reference = NameRef {
                        position: InFile::new(cont_id.file(db), source),
                        kind: if call_callees.contains(&expr_id) {
                            RefKind::Call
                        } else {
                            RefKind::Value
                        },
                    };
                    let res = sema.resolve_name_at(
                        cont_id.clone(),
                        name,
                        NameContext::Value,
                        Some(&reference),
                    );
                    collect_resolved_path(sema, res, range, collector);
                }
                _ => {}
            }
        }

        for (decl_id, decl) in lowered.data_ref().decls.iter() {
            let _: Option<()> = try {
                let name = decl.name.as_ref()?;
                let range = lowered.source_name_range(db, decl_id)?;
                check_range!(collector, range);
                collect_ident_like(name, range, collector);
            };
        }

        for (typedef_id, typedef) in lowered.data_ref().typedefs.iter() {
            let _: Option<()> = try {
                let _name = typedef.name.as_ref()?;
                let range = lowered.source_name_range(db, typedef_id)?;
                check_range!(collector, range);
                collector.tokens.add(SemaToken {
                    range,
                    tag: SemaTokenTag::Type,
                    mods: SemaTokenModifier::DECL | SemaTokenModifier::DEF,
                });
            };
        }
    }};
}

fn collect_file(
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    collector: &mut SemaTokenCollector,
) {
    let owner = sema.db.owner_table(file_id).file_owner().expect("file owner");
    let lowered = sema.db.body_with_source_map(owner);
    let hir_file = lowered.data_ref();
    let tree = sema.db.parse(file_id);
    let body = lowered.clone();

    for module_id in hir_file.module_owners() {
        let Some(range) = module_id.source(sema.db).map(|source| source.value.full_range()) else {
            continue;
        };
        check_range!(collector, range);
        collect_module(sema, module_id, collector);
    }

    for subroutine in hir_file.subroutine_owners() {
        collect_subroutine(sema, subroutine, collector);
    }
    for proc in hir_file.procs.values() {
        let owner = proc.owner;
        let body = sema.db.body_with_source_map(owner);
        collect_container_body!(sema, owner, &tree, &mut *collector, &body);
    }
    collect_container_body!(
        sema,
        sema.db.owner_table(file_id).file_owner().expect("file owner"),
        &tree,
        collector,
        &body
    );
}

fn collect_module(
    sema: &Semantics<'_, RootDb>,
    module_id: OwnerId,
    collector: &mut SemaTokenCollector,
) {
    let db = sema.db;
    let owner = module_id;
    let lowered = db.body_with_source_map(owner);
    let module = lowered.data_ref();
    let tree = db.parse(module_id.file(db));
    let body = lowered.clone();
    port::collect_port(sema, module_id, collector);

    for (instance_id, _) in module.instances.iter() {
        if let Some(range) = lowered.source_name_range(db, instance_id) {
            check_range!(collector, range);
            let sema_token =
                SemaToken { range, tag: SemaTokenTag::Instance, mods: SemaTokenModifier::empty() };
            collector.tokens.add(sema_token);
        };
    }

    let from_file = module_id.file(db).source_file_id(db);
    collect_named_param_assignments(
        sema,
        from_file,
        collector,
        module.inst_param_assigns.iter(),
        |assign_id| {
            lowered
                .ast(db, assign_id, &tree)
                .and_then(ast::ParamAssignment::as_named_param_assignment)
                .zip(lowered.source_name_range(db, assign_id))
        },
    );
    collect_named_port_connections(
        sema,
        from_file,
        collector,
        module.inst_port_conns.iter(),
        |conn_id| {
            lowered
                .ast(db, conn_id, &tree)
                .and_then(ast::PortConnection::as_named_port_connection)
                .zip(lowered.source_name_range(db, conn_id))
        },
    );

    for (_, region) in module.generate_regions.iter() {
        for item in &region.items {
            if let BodyItem::GenerateBlockOwner(generate_block_id) = item {
                collect_generate_block(sema, *generate_block_id, collector);
            }
        }
    }

    for subroutine in module.subroutine_owners() {
        collect_subroutine(sema, subroutine, collector);
    }
    for proc in module.procs.values() {
        let owner = proc.owner;
        let body = db.body_with_source_map(owner);
        collect_container_body!(sema, owner, &tree, &mut *collector, &body);
    }
    collect_container_body!(sema, module_id, &tree, collector, &body);
}

fn collect_generate_block(
    sema: &Semantics<'_, RootDb>,
    generate_block_owner: OwnerId,
    collector: &mut SemaTokenCollector,
) {
    let db = sema.db;
    let lowered = db.body_with_source_map(generate_block_owner);
    let generate_block = lowered.data_ref();
    let tree = db.parse(generate_block_owner.file(db));
    let body = lowered.clone();
    let from_file = generate_block_owner.file(db).source_file_id(db);

    for (instance_id, _) in generate_block.instances.iter() {
        if let Some(range) = lowered.source_name_range(db, instance_id) {
            check_range!(collector, range);
            let sema_token =
                SemaToken { range, tag: SemaTokenTag::Instance, mods: SemaTokenModifier::empty() };
            collector.tokens.add(sema_token);
        };
    }

    collect_named_param_assignments(
        sema,
        from_file,
        collector,
        generate_block.inst_param_assigns.iter(),
        |assign_id| {
            lowered
                .ast(db, assign_id, &tree)
                .and_then(ast::ParamAssignment::as_named_param_assignment)
                .zip(lowered.source_name_range(db, assign_id))
        },
    );
    collect_named_port_connections(
        sema,
        from_file,
        collector,
        generate_block.inst_port_conns.iter(),
        |conn_id| {
            lowered
                .ast(db, conn_id, &tree)
                .and_then(ast::PortConnection::as_named_port_connection)
                .zip(lowered.source_name_range(db, conn_id))
        },
    );

    for item in &generate_block.items {
        if let BodyItem::GenerateBlockOwner(child_owner) = item {
            collect_generate_block(sema, *child_owner, collector);
        }
    }

    for subroutine in generate_block.subroutine_owners() {
        collect_subroutine(sema, subroutine, collector);
    }

    collect_container_body!(sema, generate_block_owner, &tree, collector, &body);
}

fn collect_subroutine(
    sema: &Semantics<'_, RootDb>,
    owner: OwnerId,
    collector: &mut SemaTokenCollector,
) {
    let db = sema.db;
    let lowered = db.body_with_source_map(owner);
    let tree = db.parse(owner.file(db));

    collect_container_body!(sema, owner, &tree, collector, &lowered);
}

/// Collects named parameter assignments inside `inst_param_assigns`, resolving
/// each name against the target module. `named` projects a source range and
/// the AST assignment from an assignment id.
fn collect_named_param_assignments<'a>(
    sema: &Semantics<'_, RootDb>,
    from_file: Option<FileId>,
    collector: &mut SemaTokenCollector,
    assigns: impl Iterator<Item = (ParamAssignId, &'a ParamAssign)>,
    named: impl Fn(ParamAssignId) -> Option<(ast::NamedParamAssignment<'a>, TextRange)>,
) {
    for (assign_id, assign) in assigns {
        let ParamAssign::Named(Some(_), _) = assign else {
            continue;
        };
        let Some((named_assign, range)) = named(assign_id) else {
            continue;
        };
        check_range!(collector, range);

        let res = from_file.map_or(Resolution::Unresolved, |f| {
            resolve_named_param_assignment(sema.db, f, named_assign)
        });
        collect_resolved_path(sema, res, range, collector);
    }
}

/// Collects named port connections inside `inst_port_conns`, resolving each
/// name against the target module. `named` projects a source range and the AST
/// connection from a connection id.
fn collect_named_port_connections<'a>(
    sema: &Semantics<'_, RootDb>,
    from_file: Option<FileId>,
    collector: &mut SemaTokenCollector,
    conns: impl Iterator<Item = (PortConnId, &'a PortConn)>,
    named: impl Fn(PortConnId) -> Option<(ast::NamedPortConnection<'a>, TextRange)>,
) {
    for (conn_id, conn) in conns {
        let PortConn::Named(Some(_), _) = conn else {
            continue;
        };
        let Some((named_conn, range)) = named(conn_id) else {
            continue;
        };
        check_range!(collector, range);

        let res = from_file.map_or(Resolution::Unresolved, |f| {
            resolve_named_port_connection(sema.db, f, named_conn)
        });
        collect_resolved_path(sema, res, range, collector);
    }
}

fn collect_ident_like(
    sema: &Semantics<'_, RootDb>,
    in_cont: OwnerRef<Ident>,
    range: TextRange,
    collector: &mut SemaTokenCollector,
) -> Option<()> {
    let res = sema.resolve_name(in_cont.cont_id, &in_cont.value, NameContext::Value);
    collect_resolved_path(sema, res, range, collector)
}

fn collect_type_ref_like(
    sema: &Semantics<'_, RootDb>,
    cont_id: OwnerId,
    type_ref: &TypeRef,
    range: TextRange,
    collector: &mut SemaTokenCollector,
) -> Option<()> {
    let res = resolve_path(sema.db, cont_id, type_ref.segments(), NameContext::Type);
    collect_resolved_path(sema, res, range, collector)
}

fn collect_field_like(
    sema: &Semantics<'_, RootDb>,
    cont_id: OwnerId,
    expr_id: ExprId,
    expr: ast::Expression<'_>,
    collector: &mut SemaTokenCollector,
) -> Option<()> {
    let range = field_name_range(expr)?;
    if !collector.range.intersect(range).is_some() {
        return None;
    }
    let res = sema.expr_to_def(OwnerRef::new(cont_id, expr_id));
    collect_resolved_path(sema, res, range, collector)
}

fn field_name_range(expr: ast::Expression<'_>) -> Option<TextRange> {
    if let Some(access) = ast::MemberAccessExpression::cast(expr.syntax()) {
        return access.name()?.text_range_in(access.syntax());
    }

    if let Some(scoped) = ast::ScopedName::cast(expr.syntax()) {
        if !scoped_uses_dot(scoped) {
            return None;
        }
        return scoped_right_token(scoped)?.text_range_in(scoped.syntax());
    }

    None
}

fn scoped_right_token(scoped: ast::ScopedName<'_>) -> Option<syntax::SyntaxToken<'_>> {
    use ast::Name::*;
    match scoped.right() {
        IdentifierName(ident) => ident.identifier(),
        IdentifierSelectName(ident) => ident.identifier(),
        _ => None,
    }
}

fn scoped_uses_dot(scoped: ast::ScopedName<'_>) -> bool {
    scoped
        .syntax()
        .children()
        .filter_map(|elem| elem.as_token())
        .any(|tok| tok.kind() == syntax::Token![.])
}

fn collect_resolved_path(
    sema: &Semantics<'_, RootDb>,
    res: Resolution<DefId>,
    range: TextRange,
    collector: &mut SemaTokenCollector,
) -> Option<()> {
    let db = sema.db;
    let def_id = res.unique()?;

    if def_id.is_non_ansi_port(db) {
        let port_id = def_id.primary_origin(db).as_non_ansi_port(db)?;
        let owner = port_id.cont_id;
        let module = db.body_with_source_map(owner);
        let body = module.data_ref();
        let origins = def_id.origins(db);
        let (name, dir, ty) = resolve_port_metadata(db, &module, body, &origins)?;
        port::add_port_token(db, name, dir, ty, range, collector);
        return Some(());
    }

    match def_id.kind(db) {
        DefKind::Port => {
            let decl_id = def_id.primary_origin(db).as_decl(db)?;
            let module_id = decl_id.cont_id;
            let owner = module_id;
            let module = db.body_with_source_map(owner);
            let body = module.data_ref();
            let name = body.declarator(decl_id.value).name.as_ref()?;

            let DeclaratorParent::PortDeclId(port_declaration_id) =
                body.declarator(decl_id.value).parent
            else {
                return None;
            };
            let port_decl = module.get(port_declaration_id);
            let header = &port_decl.header;
            let (dir, ty) = (Some(header.dir()), header.ty());
            port::add_port_token(db, name, dir, ty, range, collector);
        }
        DefKind::Instance => {
            let sema_token =
                SemaToken { range, tag: SemaTokenTag::Instance, mods: SemaTokenModifier::empty() };
            collector.tokens.add(sema_token);
        }
        DefKind::Typedef => {
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
    use std::path::{Path, PathBuf};

    use base_db::{change::Change, source_root::SourceRoot};
    use insta::assert_debug_snapshot;
    use utils::text_edit::TextRange;
    use vfs::{ChangedFile, FileId, FileSet, VfsPath};

    use super::*;
    use crate::{analysis_host::AnalysisHost, test_utils::normalize_fixture_text};

    #[test]
    fn tokens_cover_generate_blocks_and_subroutine_bodies() {
        let text = r#"
module child(input logic clk);
endmodule
module top(input logic clk);
  generate
    if (1) begin : gen
      child u_gen(.clk(clk));
      typedef logic gen_ty;
    end
  endgenerate
  function automatic void drive(input logic a);
    typedef logic local_ty;
    logic local_sig;
    local_sig = a;
  endfunction
endmodule
"#;
        let (host, file_id) = setup(text);
        let tokens = host
            .make_analysis()
            .semantic_tokens(
                file_id,
                SemaTokenConfig { port: SemaTokenPortConfig { clk_rst: false, io: true } },
                None,
            )
            .unwrap();

        let token = |name: &str| -> Vec<SemaToken> {
            tokens
                .iter()
                .copied()
                .filter(|token| {
                    text.get(usize::from(token.range.start())..usize::from(token.range.end()))
                        == Some(name)
                })
                .collect()
        };

        let instance = token("u_gen");
        assert!(
            instance.iter().any(|t| t.tag == SemaTokenTag::Instance),
            "instance inside a generate block should get an Instance tag, got {instance:?}"
        );
        let typedef_decl = |name: &str| {
            token(name).iter().any(|t| {
                t.tag == SemaTokenTag::Type
                    && t.mods.contains(SemaTokenModifier::DECL)
                    && t.mods.contains(SemaTokenModifier::DEF)
            })
        };
        assert!(
            typedef_decl("gen_ty"),
            "typedef inside a generate block should get a Type|DECL|DEF token"
        );
        assert!(
            typedef_decl("local_ty"),
            "typedef inside a function body should get a Type|DECL|DEF token"
        );
    }

    fn setup(text: &str) -> (AnalysisHost, FileId) {
        let text = normalize_fixture_text(text);
        let file_id = FileId::from_raw(0);
        let path = VfsPath::new_virtual_path("/test.v".to_string());

        let mut file_set = FileSet::default();
        file_set.insert(file_id, path);
        let root = SourceRoot::new_local(file_set);

        let mut change = Change::new();
        change.set_roots(vec![root]);
        change.add_changed_file(ChangedFile::create(file_id, text.as_str()));

        let mut host = AnalysisHost::default();
        host.apply_change(change);
        (host, file_id)
    }

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/semantic_tokens/fixtures")
    }

    struct SemanticTokenFixture {
        text: String,
        config: SemaTokenConfig,
    }

    impl SemanticTokenFixture {
        fn read(path: &Path) -> Self {
            let raw =
                std::fs::read_to_string(path).unwrap_or_else(|err| panic!("read {path:?}: {err}"));
            let mut source_start = 0;
            let mut config =
                SemaTokenConfig { port: SemaTokenPortConfig { clk_rst: false, io: false } };

            for line in raw.split_inclusive('\n') {
                let line_text = line.trim_end_matches(['\r', '\n']);
                let Some(meta) = line_text.strip_prefix("//- ") else {
                    break;
                };
                source_start += line.len();

                let (key, value) = meta
                    .split_once(':')
                    .unwrap_or_else(|| panic!("invalid fixture metadata in {path:?}"));
                let value = parse_bool_metadata(value.trim(), path);
                match key.trim() {
                    "port.clk_rst" => config.port.clk_rst = value,
                    "port.io" => config.port.io = value,
                    other => panic!("unknown fixture metadata key `{other}` in {path:?}"),
                }
            }

            Self { text: raw[source_start..].to_owned(), config }
        }
    }

    fn parse_bool_metadata(value: &str, path: &Path) -> bool {
        value.parse().unwrap_or_else(|_| panic!("invalid bool metadata `{value}` in {path:?}"))
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
            let fixture = SemanticTokenFixture::read(&path);
            let text = normalize_fixture_text(&fixture.text);
            let (host, file_id) = setup(&text);
            let tokens = host
                .make_analysis()
                .semantic_tokens(
                    file_id,
                    fixture.config,
                    Some(TextRange::up_to(utils::text_edit::TextSize::of(text.as_str()))),
                )
                .unwrap();
            assert_debug_snapshot!(name, tokens);
        }
    }
}
