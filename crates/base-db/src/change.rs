use salsa::Durability;
use triomphe::Arc;
use vfs::{Change as VfsChange, ChangedFile};

use crate::{
    project::SharedProjectConfig,
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
                    db.set_source_root_id_with_durability(file_id, root_id, durability);
                    db.set_file_kind_with_durability(file_id, kind, durability);
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

            match &changed_file.change {
                VfsChange::Create(_, _) => {
                    files_changed |= files.insert(file_id);
                }
                VfsChange::Delete => {
                    files.remove(&file_id);
                    files_changed = true;
                }
                VfsChange::Modify(_, _) => {}
            }

            let text = changed_file.text().unwrap_or_else(|| Arc::from(""));
            // Salsa treats every input write as a new revision, even when the
            // value is unchanged. Rewriting kind on a body-only Modify dirties
            // every query that reads `file_kind` (workspace catalogs,
            // `unit_scope`, fold filters). Skip the write when the salsa
            // input already exists and already holds this kind.
            if !db.files().contains(&file_id) || db.file_kind(file_id) != kind {
                db.set_file_kind_with_durability(file_id, kind, durability);
            }
            db.set_file_text_with_durability(file_id, text, durability);
        }

        if files_changed {
            db.set_files_with_durability(files, Durability::HIGH);
        }
    }
}

fn durability(source_root: &SourceRoot) -> Durability {
    if source_root.is_library() { Durability::HIGH } else { Durability::LOW }
}
