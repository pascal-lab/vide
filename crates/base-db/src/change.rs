use salsa::Durability;
use triomphe::Arc;
use vfs::ChangedFile;

use crate::{
    project::{PreprocessConfig, SharedProjectConfig},
    source_db::SourceRootDb,
    source_root::{SourceRoot, SourceRootId},
};

#[derive(Debug, Default)]
pub struct Change {
    pub roots: Option<Vec<SourceRoot>>,
    pub project_config: Option<SharedProjectConfig>,
    pub changed_files: Vec<ChangedFile>,
}

impl Change {
    pub fn new() -> Self {
        Change::default()
    }

    pub fn set_roots(&mut self, roots: Vec<SourceRoot>) {
        self.roots = Some(roots);
    }

    pub fn set_project_config(&mut self, project_config: SharedProjectConfig) {
        self.project_config = Some(project_config);
    }

    pub fn add_changed_file(&mut self, changed_file: ChangedFile) {
        self.changed_files.push(changed_file)
    }

    pub fn apply(self, db: &mut dyn SourceRootDb) {
        if let Some(project_config) = self.project_config {
            db.set_project_config_with_durability(project_config, Durability::HIGH);
        }

        if let Some(roots) = self.roots {
            for (idx, root) in roots.into_iter().enumerate() {
                let root_id = SourceRootId(idx as u32);
                let durability = durability(&root);
                for file_id in root.iter() {
                    let kind = root.file_kind(&file_id);
                    let path = root
                        .path_for_file(&file_id)
                        .and_then(|path| path.as_abs_path().map(ToOwned::to_owned));
                    db.set_source_root_id_with_durability(file_id, root_id, durability);
                    db.set_file_kind_with_durability(file_id, kind, durability);
                    db.set_file_path_with_durability(file_id, path, durability);
                }
                db.set_source_root_with_durability(root_id, Arc::new(root), durability);
            }
        }

        let mut files = db.files();
        let mut files_changed = false;
        for changed_file in self.changed_files {
            let file_id = changed_file.file_id;
            let source_root_id = db.source_root_id(file_id);
            let source_root = db.source_root(source_root_id);
            let durability = durability(&source_root);
            let kind = source_root.file_kind(&file_id);
            let path = source_root
                .path_for_file(&file_id)
                .and_then(|path| path.as_abs_path().map(ToOwned::to_owned));

            match changed_file.change_kind {
                vfs::ChangeKind::Create(_, _) => {
                    files_changed |= files.insert(file_id);
                }
                vfs::ChangeKind::Delete => {
                    files.remove(&file_id);
                    files_changed = true;
                }
                vfs::ChangeKind::Modify(_, _) => {}
            }

            let text = changed_file.text().unwrap_or_else(|| Arc::from(""));
            db.set_file_kind_with_durability(file_id, kind, durability);
            db.set_file_path_with_durability(file_id, path, durability);
            db.set_file_text_with_durability(file_id, text, durability);
        }

        update_file_preprocess_configs(db, files.as_ref());

        if files_changed {
            db.set_files_with_durability(files, Durability::HIGH);
        }
    }
}

fn update_file_preprocess_configs(
    db: &mut dyn SourceRootDb,
    files: &rustc_hash::FxHashSet<vfs::FileId>,
) {
    let project_config = db.project_config();
    for file_id in files.iter().copied() {
        let source_root_id = db.source_root_id(file_id);
        let source_root = db.source_root(source_root_id);
        let profile_id = db.file_compilation_profile(file_id);
        let preprocess = if source_root.is_ignored() {
            PreprocessConfig::default()
        } else {
            project_config.preprocess_for_profile(profile_id)
        };
        db.set_file_preprocess_config_with_durability(
            file_id,
            Arc::new(preprocess),
            durability(&source_root),
        );
    }
}

fn durability(source_root: &SourceRoot) -> Durability {
    if source_root.is_library() { Durability::HIGH } else { Durability::LOW }
}
