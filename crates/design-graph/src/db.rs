//! Salsa `file_facts` over an unexpanded parse.

use base_db::{salsa, source_db::SourceRootDb};
use syntax::{SyntaxTree, SyntaxTreeOptions};
use triomphe::Arc;
use vfs::FileId;

use crate::facts::{FileFacts, extract};

#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
pub struct FileFactsKey {
    #[returns(copy)]
    pub file_id: FileId,
}

/// Workspace database that can extract unexpanded design-unit facts.
#[salsa::db]
pub trait DesignGraphDb: SourceRootDb {}

fn default_source_buffer_path(db: &dyn SourceRootDb, file_id: FileId) -> String {
    db.file_path(file_id).map(|path| path.to_string()).unwrap_or_else(|| {
        if cfg!(windows) {
            format!(r"C:\__vide_virtual__\{}", file_id.index())
        } else {
            format!("/__vide_virtual__/{}", file_id.index())
        }
    })
}

#[salsa::tracked(lru = 256, returns(clone))]
pub fn file_facts_query(db: &dyn DesignGraphDb, key: FileFactsKey) -> Arc<FileFacts> {
    let file_id = key.file_id(db);
    let text = db.file_text(file_id);
    let path = default_source_buffer_path(db, file_id);
    let name =
        db.file_path(file_id).map(|path| path.to_string()).unwrap_or_else(|| "source".into());
    let profile_id = db.file_compilation_profile(file_id);
    let predefines = db.project_config().preprocess_for_profile(profile_id).predefine_strings();
    let options = SyntaxTreeOptions {
        predefines,
        include_paths: Vec::new(),
        include_buffers: Vec::new(),
        expand_includes: false,
        collect_expected_syntax: false,
        expected_syntax_offset: None,
    };
    let tree = SyntaxTree::from_file_in_memory_with_options(&text, &name, &path, &options);
    Arc::new(extract::from_tree(file_id, &tree, &text))
}

pub fn set_file_facts_lru_capacity(db: &mut dyn DesignGraphDb, capacity: usize) {
    file_facts_query::set_lru_capacity(db, capacity);
}

impl dyn DesignGraphDb + '_ {
    pub fn file_facts(&self, file_id: FileId) -> Arc<FileFacts> {
        file_facts_query(self, FileFactsKey::new(self, file_id))
    }
}
