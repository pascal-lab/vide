use rustc_hash::FxHashSet;
use triomphe::Arc;
use vfs::vfs::{AnchoredPath, FileId};

use crate::{
    package_graph::{PackageGraph, PackageId},
    source_root::{SourceRoot, SourceRootId},
};

pub trait FileLoader {
    fn file_text(&self, file_id: FileId) -> Arc<str>;
    fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId>;
    fn relevant_packages(&self, file_id: FileId) -> Arc<FxHashSet<PackageId>>;
}

pub trait DbUpcast<T: ?Sized> {
    fn upcast(&self) -> &T;
}

// Source code and project model.
// Everything else is derived from these queries.
#[salsa::query_group(SourceDbStorage)]
pub trait SourceDb: FileLoader + std::fmt::Debug {
    // TODO: Parses the file into the syntax tree.
    // #[salsa::invoke(parse_query)]
    // fn parse(&self, file_id: FileId) -> Parse<ast::SourceFile>;

    #[salsa::input]
    fn package_graph(&self) -> Arc<PackageGraph>;
}

// Don't expose source roots to HIR, so extract them in a separate DB.
#[salsa::query_group(SourceRootDbStorage)]
pub trait SourceRootDb: SourceDb {
    #[salsa::input]
    fn file_text(&self, file_id: FileId) -> Arc<str>;

    #[salsa::input]
    fn source_root_id(&self, file_id: FileId) -> SourceRootId;

    #[salsa::input]
    fn source_root(&self, id: SourceRootId) -> Arc<SourceRoot>;

    fn package_id(&self, id: SourceRootId) -> Arc<FxHashSet<PackageId>>;
}

fn package_id(db: &dyn SourceRootDb, id: SourceRootId) -> Arc<FxHashSet<PackageId>> {
    let graph = db.package_graph();
    let res = graph
        .iter()
        .filter(|&pack_id| {
            let root_file = graph[pack_id].root_file_id;
            db.source_root_id(root_file) == id
        })
        .collect();
    Arc::new(res)
}
