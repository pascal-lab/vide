use salsa::Durability;
use triomphe::Arc;
use vfs::ChangedFile;

use crate::{
    source_db::{SourceFileKind, SourceRootDb},
    source_root::{SourceRoot, SourceRootId},
};

#[derive(Debug, Default)]
pub struct Change {
    pub roots: Option<Vec<SourceRoot>>,
    pub changed_files: Vec<ChangedFile>,
}

impl Change {
    pub fn new() -> Self {
        Change::default()
    }

    pub fn set_roots(&mut self, roots: Vec<SourceRoot>) {
        self.roots = Some(roots);
    }

    pub fn add_changed_file(&mut self, changed_file: ChangedFile) {
        self.changed_files.push(changed_file)
    }

    pub fn apply(self, db: &mut dyn SourceRootDb) {
        if let Some(roots) = self.roots {
            for (idx, root) in roots.into_iter().enumerate() {
                let root_id = SourceRootId(idx as u32);
                let durability = durability(&root);
                for file_id in root.iter() {
                    let kind = root
                        .path_for_file(&file_id)
                        .map(SourceFileKind::from_path)
                        .unwrap_or_default();
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
            let source_root = db.source_root(db.source_root_id(file_id));
            let durability = durability(&source_root);
            let kind = source_root
                .path_for_file(&file_id)
                .map(SourceFileKind::from_path)
                .unwrap_or_default();

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
            db.set_file_text_with_durability(file_id, text, durability);
        }

        if files_changed {
            db.set_files_with_durability(files, Durability::HIGH);
        }
    }
}

fn durability(source_root: &SourceRoot) -> Durability {
    if source_root.is_lib { Durability::HIGH } else { Durability::LOW }
}
