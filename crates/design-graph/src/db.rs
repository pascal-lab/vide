//! Salsa `file_facts` over an unexpanded parse.

use std::cell::Cell;

use base_db::{salsa, source_db::SourceRootDb};
use syntax::{SyntaxTree, SyntaxTreeOptions};
use triomphe::Arc;
use vfs::FileId;

use crate::{
    facts::{DeclIndex, FileFacts, extract},
    graph::{GeneratedUnits, UnitCatalog},
};

thread_local! {
    pub static SOURCE_CATALOG_RUNS: Cell<u32> = const { Cell::new(0) };
}

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
    // Profile predefines, no include expansion. This is not
    // `SyntaxTreeOptions::without_include_expansion()`: that helper ships
    // empty predefines so `source_model` stays file-local. FileFacts must
    // see the same `ifdef` view the profile will compile, or gated units
    // disappear from the name catalog. Sharing one salsa query would
    // either hide those units or make every profile edit invalidate the
    // file-local preprocessor model.
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

/// Position-free and small: must not share the parse LRU with `file_facts`.
/// A workspace larger than that LRU would otherwise re-extract every evicted
/// file's decls on the next revision, which re-parses `file_facts` with them.
#[salsa::tracked(returns(clone))]
pub fn file_decls_query(db: &dyn DesignGraphDb, key: FileFactsKey) -> Arc<DeclIndex> {
    Arc::new(file_facts_query(db, key).decls())
}

#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
pub struct UnitCatalogKey {
    #[returns(copy)]
    pub _unit: (),
}

/// L0 name catalog of source decls. T14 turns this into the production
/// source side: `source_unit_catalog(db).with_overlay(generated)`. Overlay
/// is fingerprint-keyed and is not a salsa input, so it must not enter
/// this query. Tests also use it to observe salsa backdating of
/// `file_decls`.
#[salsa::tracked(lru = 4, returns(clone))]
pub fn source_unit_catalog_query(
    db: &dyn DesignGraphDb,
    _key: UnitCatalogKey,
) -> triomphe::Arc<UnitCatalog> {
    SOURCE_CATALOG_RUNS.with(|runs| runs.set(runs.get() + 1));
    let decls: Vec<_> = db
        .files()
        .iter()
        .copied()
        .filter(|&file_id| db.file_kind(file_id).is_semantic_compilation_unit())
        .map(|file_id| db.file_decls(file_id))
        .collect();
    triomphe::Arc::new(UnitCatalog::from_decls(
        decls.iter().map(std::convert::AsRef::as_ref),
        &GeneratedUnits::default(),
    ))
}

pub fn set_file_facts_lru_capacity(db: &mut dyn DesignGraphDb, capacity: usize) {
    file_facts_query::set_lru_capacity(db, capacity);
}

impl dyn DesignGraphDb + '_ {
    pub fn file_facts(&self, file_id: FileId) -> Arc<FileFacts> {
        file_facts_query(self, FileFactsKey::new(self, file_id))
    }

    pub fn file_decls(&self, file_id: FileId) -> Arc<DeclIndex> {
        file_decls_query(self, FileFactsKey::new(self, file_id))
    }

    pub fn source_unit_catalog(&self) -> triomphe::Arc<UnitCatalog> {
        source_unit_catalog_query(self, UnitCatalogKey::new(self, ()))
    }
}
