use std::cmp::Ordering;

use base_db::{source_db::SourceRootDb, source_root::SourceRootRole};
use hir::{
    db::HirDb,
    hir_def::{Ident, module::ModuleId},
    scope::ScopeResolution,
};
use ide_db::root_db::RootDb;
use syntax::ast;
use vfs::{FileId, VfsPath};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ModuleResolution {
    Unique(ModuleId),
    Ambiguous(Vec<ModuleId>),
    Unresolved,
}

impl ModuleResolution {
    pub(crate) fn unique(&self) -> Option<ModuleId> {
        match self {
            ModuleResolution::Unique(module_id) => Some(*module_id),
            ModuleResolution::Ambiguous(_) | ModuleResolution::Unresolved => None,
        }
    }
}

pub(crate) fn resolve_instantiation_target(
    db: &RootDb,
    from_file: FileId,
    instantiation: ast::HierarchyInstantiation,
) -> ModuleResolution {
    let Some(name) = hir::hir_def::lower_ident_opt(instantiation.type_()) else {
        return ModuleResolution::Unresolved;
    };
    resolve_module_name(db, from_file, &name)
}

pub(crate) fn resolve_module_name(
    db: &RootDb,
    from_file: FileId,
    name: &Ident,
) -> ModuleResolution {
    let policy = ModuleResolutionPolicy::for_file(db, from_file);
    resolve_module_name_with_policy(db, name, policy)
}

fn resolve_module_name_with_policy(
    db: &RootDb,
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
            Self::Strict => ModuleResolution::Ambiguous(candidates),
            Self::BestEffortProximity { from_file } => {
                resolve_by_proximity(db, from_file, candidates)
            }
        }
    }
}

fn resolve_by_proximity(
    db: &RootDb,
    from_file: FileId,
    candidates: Vec<ModuleId>,
) -> ModuleResolution {
    let mut best_score = None;
    let mut best_modules = Vec::new();

    for module_id in candidates {
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

    match best_modules.as_slice() {
        [] => ModuleResolution::Unresolved,
        [module_id] => ModuleResolution::Unique(*module_id),
        _ => {
            best_modules.sort_by_key(|module_id| module_id.file_id.file_id().0);
            ModuleResolution::Ambiguous(best_modules)
        }
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
    use base_db::{change::Change, source_root::SourceRoot};
    use smol_str::SmolStr;
    use triomphe::Arc;
    use utils::lines::LineEnding;
    use vfs::{ChangeKind, ChangedFile, FileId, FileSet};

    use super::*;

    fn db_with_root(files: &[(&str, &str)], root: impl FnOnce(FileSet) -> SourceRoot) -> RootDb {
        let mut db = RootDb::new(None);
        let mut file_set = FileSet::default();
        let mut change = Change::new();

        for (idx, (path, text)) in files.iter().enumerate() {
            let file_id = FileId(idx as u32);
            file_set.insert(file_id, VfsPath::new_virtual_path((*path).to_owned()));
            change.add_changed_file(ChangedFile {
                file_id,
                change_kind: ChangeKind::Create(Arc::from(*text), LineEnding::Unix),
            });
        }

        change.set_roots(vec![root(file_set)]);
        db.apply_change(change);
        db
    }

    fn child_name() -> Ident {
        SmolStr::new("child")
    }

    #[test]
    fn best_effort_resolves_duplicate_module_by_nearest_directory() {
        let db = db_with_root(
            &[
                ("/project/a/child.sv", "module child; endmodule\n"),
                ("/project/a/top.sv", "module top; child u(); endmodule\n"),
                ("/project/b/child.sv", "module child; endmodule\n"),
            ],
            SourceRoot::new_best_effort_index,
        );

        let ModuleResolution::Unique(module_id) =
            resolve_module_name(&db, FileId(1), &child_name())
        else {
            panic!("expected nearest child module to be selected");
        };

        assert_eq!(module_id.file_id.file_id(), FileId(0));
    }

    #[test]
    fn best_effort_keeps_tied_duplicate_modules_ambiguous() {
        let db = db_with_root(
            &[
                ("/project/a/child.sv", "module child; endmodule\n"),
                ("/project/b/child.sv", "module child; endmodule\n"),
                ("/project/top.sv", "module top; child u(); endmodule\n"),
            ],
            SourceRoot::new_best_effort_index,
        );

        let ModuleResolution::Ambiguous(candidates) =
            resolve_module_name(&db, FileId(2), &child_name())
        else {
            panic!("expected equally near child modules to remain ambiguous");
        };
        let candidate_files =
            candidates.into_iter().map(|module_id| module_id.file_id.file_id()).collect::<Vec<_>>();

        assert_eq!(candidate_files, vec![FileId(0), FileId(1)]);
    }

    #[test]
    fn configured_roots_do_not_use_proximity_to_resolve_duplicate_modules() {
        let db = db_with_root(
            &[
                ("/project/a/child.sv", "module child; endmodule\n"),
                ("/project/a/top.sv", "module top; child u(); endmodule\n"),
                ("/project/b/child.sv", "module child; endmodule\n"),
            ],
            SourceRoot::new_local,
        );

        let ModuleResolution::Ambiguous(candidates) =
            resolve_module_name(&db, FileId(1), &child_name())
        else {
            panic!("expected configured root to preserve duplicate module ambiguity");
        };
        let candidate_files =
            candidates.into_iter().map(|module_id| module_id.file_id.file_id()).collect::<Vec<_>>();

        assert_eq!(candidate_files, vec![FileId(0), FileId(2)]);
    }
}
