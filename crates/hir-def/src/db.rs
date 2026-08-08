use std::ops::Deref;

use utils::text_edit::TextSize;

use base_db::salsa;
use preproc_expand::{db::PreprocDb, file::HirFileId};
use triomphe::Arc;

use crate::{
    ast_id_map::{self, AstIdMap, SyntaxFileId},
    body::{self, Body},
    def_id::{self, DefinitionTable},
    design_map,
    design_map::PackageExports,
    diagnostics,
    item_tree::{self, ItemTree, ItemTreeItem, Signature},
    owner::{self, OwnerId, OwnerTable},
    scope,
    source_map::Lowered,
    source_projection::{self, SourceProjection},
    subroutine::Subroutine,
    unit_index,
};

#[salsa::db]
pub trait HirDefDb: PreprocDb {}

// Salsa attaches tracked query methods to `dyn Db`; keep the lower-layer
// surface available on composed database trait objects without forwarding.
impl Deref for dyn HirDefDb {
    type Target = dyn PreprocDb;

    fn deref(&self) -> &Self::Target {
        self
    }
}

impl dyn HirDefDb + '_ {
    /// Canonical Salsa identity for a preprocessing-layer HIR file.
    pub fn syntax_file(&self, file_id: HirFileId) -> SyntaxFileId {
        SyntaxFileId::new(self, file_id)
    }

    /// Stable per-file AST node ids in depth-first preorder; source-side
    /// identity is deliberately separate from owner identity.
    pub fn ast_id_map(&self, file_id: HirFileId) -> Arc<AstIdMap> {
        ast_id_map::ast_id_map(self, self.syntax_file(file_id))
    }

    /// Canonical structural owners, built by the same traversal as ItemTree.
    pub fn owner_table(&self, file_id: HirFileId) -> Arc<OwnerTable> {
        owner::owner_table(self, self.syntax_file(file_id))
    }

    pub fn body_with_source_map(&self, owner: OwnerId) -> Arc<Lowered<Body>> {
        body::body_with_source_map(self, owner)
    }

    pub fn body(&self, owner: OwnerId) -> Arc<Body> {
        self.body_with_source_map(owner).data()
    }

    pub(crate) fn definition_table(&self, owner: OwnerId) -> Arc<DefinitionTable> {
        def_id::definition_table(self, owner)
    }

    pub fn item_tree(&self, file_id: HirFileId) -> Arc<ItemTree> {
        item_tree::item_tree(self, self.syntax_file(file_id))
    }

    pub fn item_for_owner(&self, owner: OwnerId) -> Option<ItemTreeItem> {
        item_tree::item_for_owner(self, owner)
    }

    pub fn signature_for_owner(&self, owner: OwnerId) -> Option<Signature> {
        item_tree::signature_for_owner(self, owner)
    }

    pub fn source_projection(&self, file_id: HirFileId) -> Arc<SourceProjection> {
        source_projection::source_projection(self, self.syntax_file(file_id))
    }

    pub fn owner_region_tree(&self, owner: OwnerId) -> Arc<crate::region_tree::RegionTree> {
        crate::region_tree::owner_region_tree(self, owner)
    }

    /// All lowering diagnostics of a file, flattened across every lowering
    /// owner (file, module, block, subroutine, generate block). Diagnostics
    /// reported without a root-buffer range get a display range resolved here
    /// (see [`diagnostics`](crate::diagnostics)).
    pub fn file_lowering_diagnostics(
        &self,
        file_id: HirFileId,
    ) -> Arc<[crate::source_map::LoweringDiagnostic]> {
        diagnostics::file_lowering_diagnostics(self, self.syntax_file(file_id))
    }

    pub fn scope(&self, owner: OwnerId) -> Arc<crate::symbol::ScopeData> {
        crate::scope::scope_for(self, owner)
    }

    pub fn unit_scope(&self) -> Arc<crate::symbol::ScopeData> {
        crate::scope::unit_scope(self)
    }

    pub fn unit_index(&self) -> Arc<crate::unit_index::UnitIndex> {
        unit_index::unit_index(self)
    }

    pub fn subroutine(&self, owner: OwnerId) -> Arc<Subroutine> {
        debug_assert_eq!(owner.kind(self), crate::owner::OwnerKind::Subroutine);
        Arc::new(
            self.body(owner)
                .subroutine
                .clone()
                .expect("subroutine owner must have lowered signature"),
        )
    }

    pub fn package_export_signature(&self, package_owner: OwnerId) -> Arc<PackageExports> {
        self.package_exports(package_owner)
    }

    pub fn package_exports(&self, package_owner: OwnerId) -> Arc<PackageExports> {
        self.design_map()
            .package_exports(package_owner)
            .expect("package owner must be present in the design map")
    }

    /// `` `default_nettype `` directives of a file, in source order.
    pub(crate) fn default_nettype_directives(
        &self,
        file_id: HirFileId,
    ) -> Arc<[(TextSize, Option<crate::ty::NetKind>)]> {
        crate::ty::default_nettype_directives(self, self.syntax_file(file_id))
    }

    pub fn design_map(&self) -> Arc<crate::design_map::DesignMap> {
        crate::design_map::design_map(self)
    }
}

/// Sets the LRU capacity of the tracked HIR queries.
pub fn set_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    ast_id_map::set_ast_id_map_lru_capacity(db, capacity);
    body::set_body_lru_capacity(db, capacity);
    def_id::set_definition_table_lru_capacity(db, capacity);
    design_map::set_lru_capacity(db, capacity);
    item_tree::set_item_tree_lru_capacity(db, capacity);
    owner::set_owner_table_lru_capacity(db, capacity);
    unit_index::set_lru_capacity(db, capacity);
    scope::set_scope_lru_capacity(db, capacity);
    source_projection::set_source_projection_lru_capacity(db, capacity);
    crate::region_tree::set_region_tree_lru_capacity(db, capacity);
    crate::ty::set_default_nettype_lru_capacity(db, capacity);
}
