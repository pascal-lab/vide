use std::cmp::Ordering;

use hir::{
    base_db::{
        source_db::{SourceDb, SourceRootDb},
        source_root::SourceRootRole,
    },
    container::ScopeId,
    db::HirDb,
    def_id::DefId,
    hir_def::{
        Ident,
        declaration::Declaration,
        expr::declarator::DeclaratorParent,
        lower_ident_opt,
        module::{ModuleId, instantiation::Instantiation},
    },
    symbol::{NameContext, Resolution},
};
use syntax::{
    SyntaxAncestors,
    ast::{self, AstNode},
};
use utils::get::GetRef;
use vfs::{FileId, VfsPath};

use crate::db::{root_db::RootDb, workspace_symbol_index_db::source_root_module_index_for_root};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ModuleResolution {
    Unique(ModuleId),
    BestEffortProximity { selected: ModuleId, candidates: Vec<ModuleId> },
    Ambiguous { candidates: Vec<ModuleId>, kind: ModuleResolutionAmbiguity },
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModuleResolutionAmbiguity {
    Strict,
    BestEffortTie,
}

impl ModuleResolution {
    pub(crate) fn unique(&self) -> Option<ModuleId> {
        match self {
            ModuleResolution::Unique(module_id) => Some(*module_id),
            ModuleResolution::BestEffortProximity { selected, .. } => Some(*selected),
            ModuleResolution::Ambiguous { .. } | ModuleResolution::Unresolved => None,
        }
    }

    fn into_resolution(self) -> Resolution<ModuleId> {
        match self {
            ModuleResolution::Unique(module_id)
            | ModuleResolution::BestEffortProximity { selected: module_id, .. } => {
                Resolution::Unique(module_id)
            }
            ModuleResolution::Ambiguous { candidates, .. } => {
                Resolution::from_candidates(candidates)
            }
            ModuleResolution::Unresolved => Resolution::Unresolved,
        }
    }
}

pub(crate) fn resolve_instantiation_target(
    db: &RootDb,
    from_file: FileId,
    instantiation: ast::HierarchyInstantiation,
) -> ModuleResolution {
    let Some(name) = lower_ident_opt(instantiation.type_()) else {
        return ModuleResolution::Unresolved;
    };
    resolve_module_name(db, from_file, &name)
}

pub(crate) fn resolve_hir_instantiation_target(
    db: &RootDb,
    from_file: FileId,
    instantiation: &Instantiation,
) -> Option<ModuleId> {
    resolve_module_name(db, from_file, instantiation.module_name.as_ref()?).unique()
}

pub(crate) fn resolve_module_name(
    db: &RootDb,
    from_file: FileId,
    name: &Ident,
) -> ModuleResolution {
    let policy = ModuleResolutionPolicy::for_file(db, from_file);
    resolve_module_name_with_policy(db, name, policy)
}

pub(crate) fn resolve_named_port_connection(
    db: &RootDb,
    from_file: FileId,
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
    resolve_named_port_in_instantiation(db, from_file, instantiation, &name)
}

pub(crate) fn resolve_named_param_assignment(
    db: &RootDb,
    from_file: FileId,
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
    resolve_named_param_in_instantiation(db, from_file, instantiation, &name)
}

fn resolve_named_port_in_instantiation(
    db: &RootDb,
    from_file: FileId,
    instantiation: ast::HierarchyInstantiation,
    port_name: &Ident,
) -> Resolution<DefId> {
    resolve_instantiation_target(db, from_file, instantiation)
        .into_resolution()
        .and_then(|module_id| resolve_named_port_in_module(db, module_id, port_name))
}

fn resolve_named_param_in_instantiation(
    db: &RootDb,
    from_file: FileId,
    instantiation: ast::HierarchyInstantiation,
    param_name: &Ident,
) -> Resolution<DefId> {
    resolve_instantiation_target(db, from_file, instantiation)
        .into_resolution()
        .and_then(|module_id| resolve_named_param_in_module(db, module_id, param_name))
}

fn resolve_named_port_in_module(
    db: &RootDb,
    module_id: ModuleId,
    port_name: &Ident,
) -> Resolution<DefId> {
    Resolution::from_candidates(
        db.module_scope(module_id)
            .lookup(NameContext::Value, port_name)
            .into_candidates()
            .into_iter()
            .filter(|def_id| def_id.is_port(db)),
    )
}

pub(crate) fn resolve_named_param_in_module(
    db: &RootDb,
    module_id: ModuleId,
    param_name: &Ident,
) -> Resolution<DefId> {
    let defs = db.module_scope(module_id).lookup(NameContext::Value, param_name);
    let module = db.module(module_id);

    Resolution::from_candidates(defs.into_candidates().into_iter().filter(|def_id| {
        let Some(decl_id) = def_id.primary_origin(db).as_decl(db) else {
            return false;
        };
        if decl_id.cont_id != ScopeId::Module(module_id) {
            return false;
        }
        let DeclaratorParent::DeclarationId(declaration_id) = module.get(decl_id.value).parent
        else {
            return false;
        };
        let Declaration::ParamDecl(param_decl) = module.get(declaration_id) else {
            return false;
        };
        param_decl.kind.is_overridable()
    }))
}

fn resolve_module_name_with_policy(
    db: &RootDb,
    name: &Ident,
    policy: ModuleResolutionPolicy,
) -> ModuleResolution {
    let candidates = module_candidates(db, name);
    match candidates.as_slice() {
        [module_id] => ModuleResolution::Unique(*module_id),
        [] => ModuleResolution::Unresolved,
        _ => policy.resolve_ambiguous(db, candidates),
    }
}

fn module_candidates(db: &RootDb, name: &Ident) -> Vec<ModuleId> {
    let mut source_root_ids =
        db.files().iter().map(|&file_id| db.source_root_id(file_id)).collect::<Vec<_>>();
    source_root_ids.sort_unstable();
    source_root_ids.dedup();

    let mut candidates = Vec::new();
    for source_root_id in source_root_ids {
        let module_index = source_root_module_index_for_root(db, source_root_id);
        candidates.extend(
            module_index
                .module_definitions(name)
                .iter()
                .map(|module| (module.file_id, module.name_range.start(), module.module_id)),
        );
    }

    candidates.sort_by_key(|(file_id, name_start, _)| (file_id.index(), *name_start));
    candidates.dedup_by_key(|(_, _, module_id)| *module_id);
    candidates.into_iter().map(|(_, _, module_id)| module_id).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModuleResolutionPolicy {
    Strict,
    // Best-effort indexing has no manifest-backed compilation profile. Use
    // source proximity as an IDE-only tie breaker, but only when it produces a
    // unique candidate; configured roots keep duplicate module names ambiguous.
    BestEffortProximity { from_file: FileId },
}

impl ModuleResolutionPolicy {
    fn for_file(db: &RootDb, file_id: FileId) -> Self {
        match source_root_role(db, file_id) {
            SourceRootRole::BestEffortIndex => Self::BestEffortProximity { from_file: file_id },
            SourceRootRole::Local | SourceRootRole::Library | SourceRootRole::Ignored => {
                Self::Strict
            }
        }
    }

    fn resolve_ambiguous(self, db: &RootDb, candidates: Vec<ModuleId>) -> ModuleResolution {
        match self {
            Self::Strict => {
                ModuleResolution::Ambiguous { candidates, kind: ModuleResolutionAmbiguity::Strict }
            }
            Self::BestEffortProximity { from_file } => {
                resolve_by_proximity(db, from_file, candidates)
            }
        }
    }
}

fn resolve_by_proximity(
    db: &RootDb,
    from_file: FileId,
    mut candidates: Vec<ModuleId>,
) -> ModuleResolution {
    let mut best_score = None;
    let mut best_modules = Vec::new();

    for module_id in candidates.iter().copied() {
        let Some(score_file) = module_id.file_id.source_file_id(db) else {
            continue;
        };
        let score = ProximityScore::new(db, from_file, score_file);
        match best_score {
            None => {
                best_score = Some(score);
                best_modules.push(module_id);
            }
            Some(best) => match score.preference_cmp(&best) {
                Ordering::Greater => {
                    best_score = Some(score);
                    best_modules.clear();
                    best_modules.push(module_id);
                }
                Ordering::Equal => best_modules.push(module_id),
                Ordering::Less => {}
            },
        }
    }

    candidates.sort_by_key(|module_id| {
        module_id.file_id.source_file_id(db).map_or(u32::MAX, FileId::index)
    });

    match best_modules.as_slice() {
        [] => ModuleResolution::Unresolved,
        [selected] => ModuleResolution::BestEffortProximity { selected: *selected, candidates },
        _ => ModuleResolution::Ambiguous {
            candidates,
            kind: ModuleResolutionAmbiguity::BestEffortTie,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProximityScore {
    same_file: bool,
    common_dir_depth: usize,
    same_source_root: bool,
}

impl ProximityScore {
    fn new(db: &RootDb, from_file: FileId, candidate_file: FileId) -> Self {
        Self {
            same_file: from_file == candidate_file,
            common_dir_depth: common_dir_depth(
                file_path(db, from_file),
                file_path(db, candidate_file),
            ),
            same_source_root: db.source_root_id(from_file) == db.source_root_id(candidate_file),
        }
    }

    fn preference_cmp(&self, other: &Self) -> Ordering {
        // Prefer exact file matches, then nearest directory, then source-root locality.
        self.same_file
            .cmp(&other.same_file)
            .then_with(|| self.common_dir_depth.cmp(&other.common_dir_depth))
            .then_with(|| self.same_source_root.cmp(&other.same_source_root))
    }
}

fn source_root_role(db: &RootDb, file_id: FileId) -> SourceRootRole {
    let source_root_id = db.source_root_id(file_id);
    db.source_root(source_root_id).role()
}

fn file_path(db: &RootDb, file_id: FileId) -> Option<VfsPath> {
    let source_root_id = db.source_root_id(file_id);
    db.source_root(source_root_id).path_for_file(&file_id).cloned()
}

fn common_dir_depth(left: Option<VfsPath>, right: Option<VfsPath>) -> usize {
    let (Some(left), Some(right)) = (left, right) else {
        return 0;
    };
    let left = dir_ancestors(left);
    let right = dir_ancestors(right);
    left.iter().zip(right.iter()).take_while(|(left, right)| left == right).count()
}

fn dir_ancestors(path: VfsPath) -> Vec<VfsPath> {
    let mut ancestors = Vec::new();
    let mut current = path.parent();
    while let Some(path) = current {
        current = path.parent();
        ancestors.push(path);
    }
    ancestors.reverse();
    ancestors
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use hir::{
        base_db::{change::Change, source_root::SourceRoot},
        symbol::{DefKind, DefOriginLoc, Resolution},
    };
    use smol_str::SmolStr;
    use syntax::{SyntaxNodeExt, ast};
    use utils::text_edit::TextSize;
    use vfs::{ChangedFile, FileId, FileSet};

    use super::*;

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
                let result = resolve_module_name(&db, fixture.focus, &module);
                format_module_resolution(&fixture.files, result)
            }
            Query::NamedPort => {
                let offset = fixture.offset.expect("named_port query requires /*caret*/");
                let tree = db.parse_src_for_compilation(fixture.focus);
                let root = tree.root().expect("test source should parse");
                let port_conn = root
                    .find_node_at_offset::<ast::NamedPortConnection>(offset)
                    .expect("named port connection should parse at /*caret*/");
                let res = resolve_named_port_connection(&db, fixture.focus, port_conn);
                match resolution_module_id(&db, &res, DefKind::Port) {
                    Some(module_id) => format!(
                        "AnsiPort module={}",
                        file_path(&fixture.files, module_id.file_id.as_file().unwrap())
                    ),
                    None => format!("{res:?}"),
                }
            }
            Query::NamedParam => {
                let offset = fixture.offset.expect("named_param query requires /*caret*/");
                let tree = db.parse_src_for_compilation(fixture.focus);
                let root = tree.root().expect("test source should parse");
                let param_assign = root
                    .find_node_at_offset::<ast::NamedParamAssignment>(offset)
                    .expect("named parameter assignment should parse at /*caret*/");
                let res = resolve_named_param_assignment(&db, fixture.focus, param_assign);
                match resolution_module_id(&db, &res, DefKind::Param) {
                    Some(module_id) => format!(
                        "ParamDecl module={}",
                        file_path(&fixture.files, module_id.file_id.as_file().unwrap())
                    ),
                    None => format!("{res:?}"),
                }
            }
        }
    }

    fn resolution_module_id(
        db: &RootDb,
        res: &Resolution<DefId>,
        kind: DefKind,
    ) -> Option<ModuleId> {
        let def_id = res.unique()?;
        if def_id.kind(db) != kind {
            return None;
        }
        match def_id.primary_origin(db).loc(db) {
            DefOriginLoc::Decl(decl_id) => match decl_id.cont_id {
                ScopeId::Module(module_id) => Some(module_id),
                _ => None,
            },
            DefOriginLoc::NonAnsiPort(nonansi_port_id) => Some(nonansi_port_id.module_id),
            _ => None,
        }
    }

    fn format_module_resolution(files: &[(String, String)], result: ModuleResolution) -> String {
        match result {
            ModuleResolution::Unique(module_id) => {
                format!(
                    "Unique selected={}",
                    file_path(files, module_id.file_id.as_file().unwrap())
                )
            }
            ModuleResolution::BestEffortProximity { selected, candidates } => format!(
                "BestEffortProximity selected={} candidates={:?}",
                file_path(files, selected.file_id.as_file().unwrap()),
                candidate_paths(files, candidates)
            ),
            ModuleResolution::Ambiguous { candidates, kind } => {
                format!(
                    "Ambiguous kind={kind:?} candidates={:?}",
                    candidate_paths(files, candidates)
                )
            }
            ModuleResolution::Unresolved => "Unresolved".to_string(),
        }
    }

    fn candidate_paths(files: &[(String, String)], candidates: Vec<ModuleId>) -> Vec<String> {
        candidates
            .into_iter()
            .map(|module_id| file_path(files, module_id.file_id.as_file().unwrap()))
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
