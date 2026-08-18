use hir_def::{
    Ident,
    body::Body,
    container::OwnerRef,
    db::HirDefDb,
    declaration::Declaration,
    def_id::DefId,
    expr::{data_ty::DataTy, declarator::DeclaratorParent},
    lower_ident_opt,
    module::{
        ansi_port_decl_id_by_idx,
        instantiation::{Instantiation, PortConn},
        port::{PortDirection, Ports},
    },
    owner::OwnerId,
    source_map::Lowered,
    symbol::{DefOrigin, NameContext, Resolution},
    unit::ToOwner,
};
use smallvec::SmallVec;
use syntax::{
    SyntaxAncestors,
    ast::{self, AstNode},
};

use crate::db::workspace_symbol_index_db::WorkspaceSymbolIndexDb;

pub(crate) type ModuleResolution = Resolution<OwnerId>;

fn module_resolution_from_graph(
    db: &dyn HirDefDb,
    graph: &design_graph::UnitCatalog,
    name: &Ident,
) -> ModuleResolution {
    Resolution::from_candidates(
        graph.modules_named(name).into_vec().into_iter().filter_map(|unit| unit.to_owner(db)),
    )
}

pub(crate) fn resolve_instantiation_target(
    db: &dyn WorkspaceSymbolIndexDb,
    graph: &design_graph::UnitCatalog,
    instantiation: ast::HierarchyInstantiation,
) -> ModuleResolution {
    let Some(name) = lower_ident_opt(instantiation.type_()) else {
        return ModuleResolution::Unresolved;
    };
    resolve_module_name(db, graph, &name)
}

pub(crate) fn resolve_hir_instantiation_target(
    db: &dyn WorkspaceSymbolIndexDb,
    graph: &design_graph::UnitCatalog,
    instantiation: &Instantiation,
) -> Option<OwnerId> {
    resolve_module_name(db, graph, instantiation.module_name.as_ref()?).unique()
}

pub(crate) fn resolve_module_name(
    db: &dyn WorkspaceSymbolIndexDb,
    graph: &design_graph::UnitCatalog,
    name: &Ident,
) -> ModuleResolution {
    module_resolution_from_graph(db, graph, name)
}

pub(crate) fn resolve_named_port_connection(
    db: &dyn WorkspaceSymbolIndexDb,
    graph: &design_graph::UnitCatalog,
    conn: ast::NamedPortConnection,
) -> Resolution<DefId> {
    let Some(name) = lower_ident_opt(conn.name()) else {
        return Resolution::Unresolved;
    };
    let Some(instantiation) =
        SyntaxAncestors::start_from(conn.syntax()).find_map(ast::HierarchyInstantiation::cast)
    else {
        return Resolution::Unresolved;
    };
    resolve_named_port_in_instantiation(db, graph, instantiation, &name)
}

pub(crate) fn resolve_named_param_assignment(
    db: &dyn WorkspaceSymbolIndexDb,
    graph: &design_graph::UnitCatalog,
    assign: ast::NamedParamAssignment,
) -> Resolution<DefId> {
    let Some(name) = lower_ident_opt(assign.name()) else {
        return Resolution::Unresolved;
    };
    let Some(instantiation) =
        SyntaxAncestors::start_from(assign.syntax()).find_map(ast::HierarchyInstantiation::cast)
    else {
        return Resolution::Unresolved;
    };
    resolve_named_param_in_instantiation(db, graph, instantiation, &name)
}

fn resolve_named_port_in_instantiation(
    db: &dyn WorkspaceSymbolIndexDb,
    graph: &design_graph::UnitCatalog,
    instantiation: ast::HierarchyInstantiation,
    port_name: &Ident,
) -> Resolution<DefId> {
    resolve_instantiation_target(db, graph, instantiation)
        .and_then(|module_id| resolve_named_port_in_module(db, module_id, port_name))
}

fn resolve_named_param_in_instantiation(
    db: &dyn WorkspaceSymbolIndexDb,
    graph: &design_graph::UnitCatalog,
    instantiation: ast::HierarchyInstantiation,
    param_name: &Ident,
) -> Resolution<DefId> {
    resolve_instantiation_target(db, graph, instantiation)
        .and_then(|module_id| resolve_named_param_in_module(db, module_id, param_name))
}

fn resolve_named_port_in_module(
    db: &dyn WorkspaceSymbolIndexDb,
    module_id: OwnerId,
    port_name: &Ident,
) -> Resolution<DefId> {
    Resolution::from_candidates(
        db.scope(module_id)
            .lookup(NameContext::Value, port_name)
            .into_candidates()
            .into_iter()
            .filter(|def_id| def_id.is_port(db)),
    )
}

/// Resolves the port connected by `conn` at position `idx` inside
/// `target_module_id` to its `DefId`. Ordered and empty connections select the
/// port by position; named connections look the name up in the target module
/// scope.
pub(crate) fn resolve_connection_port(
    db: &dyn WorkspaceSymbolIndexDb,
    target_module_id: OwnerId,
    conn: &PortConn,
    idx: usize,
) -> Resolution<DefId> {
    match conn {
        PortConn::Empty | PortConn::Ordered(_) => {
            let module = db.body_with_source_map(target_module_id);
            match &module.ports {
                Ports::NonAnsi { ports, .. } => {
                    let Some((port_id, _)) = ports.iter().nth(idx) else {
                        return Resolution::Unresolved;
                    };
                    Resolution::Unique(DefId::from_source(
                        db,
                        OwnerRef::new(target_module_id, port_id),
                    ))
                }
                Ports::Ansi(_) => {
                    let Some(port_decl_id) = ansi_port_decl_id_by_idx(&module, idx) else {
                        return Resolution::Unresolved;
                    };
                    let Some(decl_id) = module.get(port_decl_id).decls.clone().next() else {
                        return Resolution::Unresolved;
                    };
                    Resolution::Unique(DefId::from_source(
                        db,
                        OwnerRef::new(target_module_id, decl_id),
                    ))
                }
            }
        }
        PortConn::Named(Some(name), _) => resolve_named_port_in_module(db, target_module_id, name),
        PortConn::Named(None, _) | PortConn::Wildcard => Resolution::Unresolved,
    }
}

/// Resolves the name, direction and type of the port a `DefId` refers to.
/// Works for both ANSI and non-ANSI ports: the metadata is derived from the
/// port declaration, found either among the def's origins or — for non-ANSI
/// ports declared in the body under a name different from their port label —
/// through the port's internal references.
pub(crate) fn resolve_port_metadata<'a>(
    db: &dyn HirDefDb,
    module: &'a Lowered<Body>,
    body: &'a Body,
    defs: &[DefOrigin],
) -> Option<(&'a Ident, Option<PortDirection>, DataTy)> {
    let mut origins: SmallVec<[DefOrigin; 8]> = SmallVec::new();
    origins.extend(defs.iter().cloned());

    if let Some(port_id) = defs.iter().find_map(|origin| origin.as_non_ansi_port(db)) {
        let scope = db.scope(port_id.cont_id);
        if let Some(refs) = module.get(port_id.value).refs.clone() {
            for ref_id in refs {
                let Some(name) = module.get(ref_id).ident.as_ref() else {
                    continue;
                };
                for def in scope.lookup(NameContext::Value, name).into_candidates() {
                    origins.extend(def.origins(db));
                }
            }
        }
    }

    let port_decl_id =
        origins.iter().filter_map(|origin| origin.as_decl(db)).map(|decl_id| decl_id.value).find(
            |decl_id| matches!(body.decls[*decl_id].parent, DeclaratorParent::PortDeclId(_)),
        )?;
    let data_decl_id =
        origins.iter().filter_map(|origin| origin.as_decl(db)).map(|decl_id| decl_id.value).find(
            |decl_id| matches!(body.decls[*decl_id].parent, DeclaratorParent::DeclarationId(_)),
        );

    let port_decl = &body.decls[port_decl_id];
    let name = defs
        .iter()
        .find_map(|origin| origin.as_non_ansi_port(db))
        .and_then(|port_id| module.get(port_id.value).label.as_ref())
        .or(port_decl.name.as_ref())?;
    let port_declaration = match port_decl.parent {
        DeclaratorParent::PortDeclId(port_declaration_id) => module.get(port_declaration_id),
        _ => return None,
    };
    let header = &port_declaration.header;
    let dir = Some(header.dir());
    let ty = if let Some(data_decl_id) = data_decl_id {
        let data_decl = &body.decls[data_decl_id];
        match data_decl.parent {
            DeclaratorParent::DeclarationId(declaration_id) => {
                let declaration = &body.declarations[declaration_id];
                declaration.ty()
            }
            _ => return None,
        }
    } else {
        header.ty()
    };

    Some((name, dir, ty))
}

pub(crate) fn resolve_named_param_in_module(
    db: &dyn WorkspaceSymbolIndexDb,
    module_id: OwnerId,
    param_name: &Ident,
) -> Resolution<DefId> {
    let defs = db.scope(module_id).lookup(NameContext::Value, param_name);
    let body = db.body_with_source_map(module_id);

    Resolution::from_candidates(defs.into_candidates().into_iter().filter(|def_id| {
        let Some(decl_id) = def_id.primary_origin(db).as_decl(db) else {
            return false;
        };
        if decl_id.cont_id != module_id {
            return false;
        }
        let DeclaratorParent::DeclarationId(declaration_id) = body.decls[decl_id.value].parent
        else {
            return false;
        };
        let Declaration::ParamDecl(param_decl) = &body.declarations[declaration_id] else {
            return false;
        };
        param_decl.kind.is_overridable()
    }))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use base_db::{change::Change, source_root::SourceRoot};
    use hir_def::symbol::{DefKind, Resolution};
    use smol_str::SmolStr;
    use syntax::{SyntaxNodeExt, ast};
    use utils::text_edit::TextSize;
    use vfs::{ChangedFile, FileId, FileSet, VfsPath};

    use super::*;
    use crate::db::root_db::RootDb;

    fn db_with_root(
        files: &[(String, String)],
        root: impl FnOnce(FileSet) -> SourceRoot,
    ) -> RootDb {
        let mut db = RootDb::new(None);
        let mut file_set = FileSet::default();
        let mut change = Change::new();

        for (idx, (path, text)) in files.iter().enumerate() {
            let file_id = FileId::from_raw(idx as u32);
            file_set.insert(file_id, VfsPath::new_virtual_path(path.clone()));
            change.add_changed_file(ChangedFile::create(file_id, text.as_str()));
        }

        change.set_roots(vec![root(file_set)]);
        db.apply_change(change);
        db
    }

    enum RootKind {
        BestEffort,
        Local,
    }

    enum Query {
        Module(SmolStr),
        NamedPort,
        NamedParam,
    }

    struct ResolutionFixture {
        root: RootKind,
        query: Query,
        focus: FileId,
        offset: Option<TextSize>,
        files: Vec<(String, String)>,
    }

    impl ResolutionFixture {
        fn read(path: &Path) -> Self {
            let raw = std::fs::read_to_string(path)
                .unwrap_or_else(|err| panic!("failed to read fixture {}: {err}", path.display()));
            let mut root = None;
            let mut query = None;
            let mut focus_path = None;
            let mut files: Vec<(String, String)> = Vec::new();
            let mut current_path: Option<String> = None;
            let mut current_text = String::new();
            let mut focus_index = None;
            let mut offset = None;

            for line in raw.lines() {
                let Some(meta) = line.strip_prefix("//- ") else {
                    current_text.push_str(line);
                    current_text.push('\n');
                    continue;
                };

                let (key, value) = meta
                    .split_once(':')
                    .unwrap_or_else(|| panic!("invalid fixture metadata in {}", path.display()));
                let value = value.trim();
                match key.trim() {
                    "root" => {
                        root = Some(match value {
                            "best_effort" => RootKind::BestEffort,
                            "local" => RootKind::Local,
                            other => panic!("unknown root kind `{other}` in {}", path.display()),
                        });
                    }
                    "query" => {
                        query = Some(match value {
                            value if let Some(module) = value.strip_prefix("module ") => {
                                Query::Module(SmolStr::new(module))
                            }
                            "named_port" => Query::NamedPort,
                            "named_param" => Query::NamedParam,
                            other => panic!("unknown query `{other}` in {}", path.display()),
                        });
                    }
                    "focus" => focus_path = Some(value.to_owned()),
                    "file" => {
                        if let Some(file_path) = current_path.take() {
                            let file_index = files.len();
                            if focus_path.as_deref() == Some(file_path.as_str()) {
                                focus_index = Some(file_index);
                            }
                            let clean_text = strip_caret(&current_text, &mut offset);
                            files.push((file_path, clean_text));
                            current_text.clear();
                        }
                        current_path = Some(value.to_owned());
                    }
                    other => panic!("unknown metadata key `{other}` in {}", path.display()),
                }
            }

            if let Some(file_path) = current_path.take() {
                let file_index = files.len();
                if focus_path.as_deref() == Some(file_path.as_str()) {
                    focus_index = Some(file_index);
                }
                let clean_text = strip_caret(&current_text, &mut offset);
                files.push((file_path, clean_text));
            }

            ResolutionFixture {
                root: root.unwrap_or_else(|| panic!("missing root in {}", path.display())),
                query: query.unwrap_or_else(|| panic!("missing query in {}", path.display())),
                focus: FileId::from_raw(
                    focus_index
                        .unwrap_or_else(|| panic!("missing focus file in {}", path.display()))
                        as u32,
                ),
                offset,
                files,
            }
        }
    }

    fn strip_caret(text: &str, offset: &mut Option<TextSize>) -> String {
        const CARET: &str = "/*caret*/";
        let Some(marker_offset) = text.find(CARET) else {
            return text.to_owned();
        };
        assert!(
            offset.is_none(),
            "only one caret marker is allowed across module resolution fixture files"
        );
        *offset = Some(TextSize::from(marker_offset as u32));
        text.replace(CARET, "")
    }

    fn fixture_snapshot(fixture: ResolutionFixture) -> String {
        let db = match fixture.root {
            RootKind::BestEffort => db_with_root(&fixture.files, SourceRoot::new_best_effort_index),
            RootKind::Local => db_with_root(&fixture.files, SourceRoot::new_local),
        };

        match fixture.query {
            Query::Module(module) => {
                let result = resolve_module_name(&db, &hir_def::unit::test_graph(&db), &module);
                format_module_resolution(&db, &fixture.files, result)
            }
            Query::NamedPort => {
                let offset = fixture.offset.expect("named_port query requires /*caret*/");
                let tree = db.parse_src_for_compilation(fixture.focus);
                let root = tree.root();
                let port_conn = root
                    .find_node_at_offset::<ast::NamedPortConnection>(offset)
                    .expect("named port connection should parse at /*caret*/");
                let res =
                    resolve_named_port_connection(&db, &hir_def::unit::test_graph(&db), port_conn);
                format_def_resolution(&db, &fixture.files, &res, DefKind::Port, "AnsiPort")
            }
            Query::NamedParam => {
                let offset = fixture.offset.expect("named_param query requires /*caret*/");
                let tree = db.parse_src_for_compilation(fixture.focus);
                let root = tree.root();
                let param_assign = root
                    .find_node_at_offset::<ast::NamedParamAssignment>(offset)
                    .expect("named parameter assignment should parse at /*caret*/");
                let res = resolve_named_param_assignment(
                    &db,
                    &hir_def::unit::test_graph(&db),
                    param_assign,
                );
                format_def_resolution(&db, &fixture.files, &res, DefKind::Param, "ParamDecl")
            }
        }
    }

    fn resolution_module_id(
        db: &RootDb,
        res: &Resolution<DefId>,
        kind: DefKind,
    ) -> Option<OwnerId> {
        let def_id = res.unique()?;
        if def_id.kind(db) != kind {
            return None;
        }
        Some(def_id.container_id(db))
    }

    fn format_def_resolution(
        db: &RootDb,
        files: &[(String, String)],
        res: &Resolution<DefId>,
        kind: DefKind,
        unique_label: &str,
    ) -> String {
        match resolution_module_id(db, res, kind) {
            Some(module_id) => format!(
                "{unique_label} module={}",
                file_path(files, module_id.file(db).as_file().unwrap())
            ),
            None => match res {
                Resolution::Ambiguous(candidates) => {
                    let owners = candidates
                        .iter()
                        .filter(|def_id| def_id.kind(db) == kind)
                        .map(|def_id| def_id.container_id(db))
                        .collect();
                    format!("Ambiguous candidates={:?}", candidate_paths(db, files, owners))
                }
                other => format!("{other:?}"),
            },
        }
    }

    fn format_module_resolution(
        db: &RootDb,
        files: &[(String, String)],
        result: ModuleResolution,
    ) -> String {
        match result {
            ModuleResolution::Unique(module_id) => {
                format!(
                    "Unique selected={}",
                    file_path(files, module_id.file(db).as_file().unwrap())
                )
            }
            ModuleResolution::Ambiguous(candidates) => {
                format!(
                    "Ambiguous candidates={:?}",
                    candidate_paths(db, files, candidates.into_iter().collect())
                )
            }
            ModuleResolution::Unresolved => "Unresolved".to_string(),
        }
    }

    fn candidate_paths(
        db: &RootDb,
        files: &[(String, String)],
        candidates: Vec<OwnerId>,
    ) -> Vec<String> {
        candidates
            .into_iter()
            .map(|module_id| file_path(files, module_id.file(db).as_file().unwrap()))
            .collect()
    }
    fn file_path(files: &[(String, String)], file_id: FileId) -> String {
        files
            .get(file_id.index() as usize)
            .map(|(path, _)| path.clone())
            .unwrap_or_else(|| format!("<unknown {:?}>", file_id))
    }

    #[test]
    fn module_resolution_fixtures() {
        insta::glob!("module_resolution/fixtures/*.sv", |path| {
            let fixture = ResolutionFixture::read(path);
            insta::assert_snapshot!(fixture_snapshot(fixture));
        });
    }
}
