use std::cmp::Ordering;

use hir::{
    base_db::{source_db::SourceRootDb, source_root::SourceRootRole},
    container::InModule,
    db::HirDb,
    hir_def::{
        Ident,
        declaration::Declaration,
        expr::declarator::DeclaratorParent,
        lower_ident_opt,
        module::{ModuleId, instantiation::Instantiation},
    },
    scope::{ModuleEntry, ScopeResolution},
};
use syntax::{
    SyntaxAncestors,
    ast::{self, AstNode},
};
use utils::get::GetRef;
use vfs::{FileId, VfsPath};

use crate::PathResolution;

/// Database capabilities required by source-level module resolution.
pub trait SemanticDb: HirDb + SourceRootDb {}

impl<T> SemanticDb for T where T: HirDb + SourceRootDb {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleResolution {
    Unique(ModuleId),
    BestEffortProximity { selected: ModuleId, candidates: Vec<ModuleId> },
    Ambiguous { candidates: Vec<ModuleId>, kind: ModuleResolutionAmbiguity },
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleResolutionAmbiguity {
    Strict,
    BestEffortTie,
}

impl ModuleResolution {
    pub fn unique(&self) -> Option<ModuleId> {
        match self {
            ModuleResolution::Unique(module_id) => Some(*module_id),
            ModuleResolution::BestEffortProximity { selected, .. } => Some(*selected),
            ModuleResolution::Ambiguous { .. } | ModuleResolution::Unresolved => None,
        }
    }
}

pub fn resolve_instantiation_target<DB: SemanticDb>(
    db: &DB,
    from_file: FileId,
    instantiation: ast::HierarchyInstantiation,
) -> ModuleResolution {
    let Some(name) = lower_ident_opt(instantiation.type_()) else {
        return ModuleResolution::Unresolved;
    };
    resolve_module_name(db, from_file, &name)
}

pub fn resolve_hir_instantiation_target<DB: SemanticDb>(
    db: &DB,
    from_file: FileId,
    instantiation: &Instantiation,
) -> Option<ModuleId> {
    resolve_module_name(db, from_file, instantiation.module_name.as_ref()?).unique()
}

pub fn resolve_module_name<DB: SemanticDb>(
    db: &DB,
    from_file: FileId,
    name: &Ident,
) -> ModuleResolution {
    let policy = ModuleResolutionPolicy::for_file(db, from_file);
    resolve_module_name_with_policy(db, name, policy)
}

pub fn resolve_named_port_connection<DB: SemanticDb>(
    db: &DB,
    from_file: FileId,
    conn: ast::NamedPortConnection,
) -> Option<PathResolution> {
    let name = lower_ident_opt(conn.name())?;
    let instantiation =
        SyntaxAncestors::start_from(conn.syntax()).find_map(ast::HierarchyInstantiation::cast)?;
    resolve_named_port_in_instantiation(db, from_file, instantiation, &name)
}

pub fn resolve_named_param_assignment<DB: SemanticDb>(
    db: &DB,
    from_file: FileId,
    assign: ast::NamedParamAssignment,
) -> Option<PathResolution> {
    let name = lower_ident_opt(assign.name())?;
    let instantiation =
        SyntaxAncestors::start_from(assign.syntax()).find_map(ast::HierarchyInstantiation::cast)?;
    resolve_named_param_in_instantiation(db, from_file, instantiation, &name)
}

fn resolve_named_port_in_instantiation<DB: SemanticDb>(
    db: &DB,
    from_file: FileId,
    instantiation: ast::HierarchyInstantiation,
    port_name: &Ident,
) -> Option<PathResolution> {
    let target_module_id = resolve_instantiation_target(db, from_file, instantiation).unique()?;
    resolve_named_port_in_module(db, target_module_id, port_name)
}

fn resolve_named_param_in_instantiation<DB: SemanticDb>(
    db: &DB,
    from_file: FileId,
    instantiation: ast::HierarchyInstantiation,
    param_name: &Ident,
) -> Option<PathResolution> {
    let target_module_id = resolve_instantiation_target(db, from_file, instantiation).unique()?;
    resolve_named_param_in_module(db, target_module_id, param_name)
}

fn resolve_named_port_in_module<DB: SemanticDb>(
    db: &DB,
    module_id: ModuleId,
    port_name: &Ident,
) -> Option<PathResolution> {
    let entry = db.module_scope(module_id).get(port_name)?;
    if matches!(entry, ModuleEntry::AnsiPortEntry(_) | ModuleEntry::NonAnsiPortEntry(_)) {
        Some(PathResolution::from(InModule::new(module_id, entry)))
    } else {
        None
    }
}

fn resolve_named_param_in_module<DB: SemanticDb>(
    db: &DB,
    module_id: ModuleId,
    param_name: &Ident,
) -> Option<PathResolution> {
    let ModuleEntry::DeclId(decl_id) = db.module_scope(module_id).get(param_name)? else {
        return None;
    };
    let module = db.module(module_id);
    if let DeclaratorParent::DeclarationId(declaration_id) = module.get(decl_id).parent
        && let Declaration::ParamDecl(param_decl) = module.get(declaration_id)
        && param_decl.kind.is_overridable()
    {
        Some(PathResolution::ParamDecl(InModule::new(module_id, decl_id)))
    } else {
        None
    }
}

fn resolve_module_name_with_policy<DB: SemanticDb>(
    db: &DB,
    name: &Ident,
    policy: ModuleResolutionPolicy,
) -> ModuleResolution {
    match db.unit_scope().resolve_module(name) {
        ScopeResolution::Unique(module_id) => ModuleResolution::Unique(module_id),
        ScopeResolution::Unresolved => ModuleResolution::Unresolved,
        ScopeResolution::Ambiguous(candidates) => {
            policy.resolve_ambiguous(db, candidates.into_vec())
        }
    }
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
    fn for_file<DB: SemanticDb>(db: &DB, file_id: FileId) -> Self {
        match source_root_role(db, file_id) {
            SourceRootRole::BestEffortIndex => Self::BestEffortProximity { from_file: file_id },
            SourceRootRole::Local | SourceRootRole::Library | SourceRootRole::Ignored => {
                Self::Strict
            }
        }
    }

    fn resolve_ambiguous<DB: SemanticDb>(
        self,
        db: &DB,
        candidates: Vec<ModuleId>,
    ) -> ModuleResolution {
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

fn resolve_by_proximity<DB: SemanticDb>(
    db: &DB,
    from_file: FileId,
    mut candidates: Vec<ModuleId>,
) -> ModuleResolution {
    let mut best_score = None;
    let mut best_modules = Vec::new();

    for module_id in candidates.iter().copied() {
        let score = ProximityScore::new(db, from_file, module_id.file_id.file_id());
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

    candidates.sort_by_key(|module_id| module_id.file_id.file_id().0);

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
    fn new<DB: SemanticDb>(db: &DB, from_file: FileId, candidate_file: FileId) -> Self {
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

fn source_root_role<DB: SemanticDb>(db: &DB, file_id: FileId) -> SourceRootRole {
    let source_root_id = db.source_root_id(file_id);
    db.source_root(source_root_id).role()
}

fn file_path<DB: SemanticDb>(db: &DB, file_id: FileId) -> Option<VfsPath> {
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
