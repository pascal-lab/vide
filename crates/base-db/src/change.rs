use salsa::Durability;
use triomphe::Arc;
use vfs::vfs::FileId;

use crate::{
    package_graph::PackageGraph,
    source_db::SourceRootDb,
    source_root::{SourceRoot, SourceRootId},
};

#[derive(Debug, Default)]
pub struct Change {
    pub roots: Option<Vec<SourceRoot>>,
    pub changed_files: Vec<(FileId, Option<Arc<str>>)>,
    pub package_graph: Option<PackageGraph>,
}

impl Change {
    pub fn new() -> Self {
        Change::default()
    }

    pub fn set_roots(&mut self, roots: Vec<SourceRoot>) {
        self.roots = Some(roots);
    }

    pub fn add_changed_file(&mut self, file_id: FileId, new_text: Option<Arc<str>>) {
        self.changed_files.push((file_id, new_text))
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

        for (file_id, new_text) in self.changed_files {
            let source_root_id = db.source_root_id(file_id);
            let source_root = db.source_root(source_root_id);
            let durability = durability(&source_root);
            // cannot remove a file, so reset it
            let text = new_text.unwrap_or_else(|| Arc::from(""));
            db.set_file_text_with_durability(file_id, text, durability);
        }

        if let Some(package_graph) = self.package_graph {
            db.set_package_graph_with_durability(Arc::new(package_graph), Durability::HIGH);
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
