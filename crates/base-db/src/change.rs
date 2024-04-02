use salsa::Durability;
use triomphe::Arc;
use vfs::vfs::ChangedFile;

use crate::{
    source_db::{edit_syntax_tree, parse_source, SourceRootDb},
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
                    db.set_source_root_id_with_durability(file_id, root_id, durability);
                }
                db.set_source_root_with_durability(root_id, Arc::new(root), durability);
            }
        }

        if self.changed_files.iter().any(|it| it.is_created_or_deleted()) {
            let mut files = db.files();
            self.changed_files.iter().for_each(|it| {
                use vfs::vfs::ChangeKind;
                match it.change_kind {
                    ChangeKind::Create(_) => {
                        files.insert(it.file_id);
                    }
                    ChangeKind::Delete => {
                        files.remove(&it.file_id);
                    }
                    ChangeKind::Modify(_, _) => {}
                }
            });
            db.set_files(files);
        }

        for changed_file in self.changed_files {
            let file_id = changed_file.file_id;
            let source_root = db.source_root(db.source_root_id(file_id));
            let durability = durability(&source_root);

            let file_exists = changed_file.exists();
            edit_syntax_tree(db, changed_file, durability);
            if file_exists {
                parse_source(db, file_id);
            }
        }
    }
}

fn durability(source_root: &SourceRoot) -> Durability {
    if source_root.is_lib {
        Durability::HIGH
    } else {
        Durability::LOW
    }
}
