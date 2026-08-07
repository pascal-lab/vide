use hir_def::{
    ast_id_map::SourceAstId,
    container::{InFile, OwnerRef},
    expr::ExprId,
    module::instantiation::{InstanceId, InstantiationId, PortConnId},
};
use preproc_expand::file::HirFileId;
use syntax::{
    SyntaxNode,
    ast::{self, AstNode},
};

use super::SemanticsImpl;

fn source_ast_id(
    db: &dyn hir_def::db::HirDefDb,
    file_id: HirFileId,
    node: SyntaxNode<'_>,
) -> Option<SourceAstId> {
    let tree = db.parse(file_id);
    db.ast_id_map(file_id).id_of_node_in_tree(&tree, node)
}

impl SemanticsImpl<'_> {
    pub fn resolve_instance(
        &self,
        file_id: HirFileId,
        instance: ast::HierarchicalInstance,
    ) -> Option<OwnerRef<InstanceId>> {
        let db = self.db;
        let owner = self.find_container(InFile::new(file_id, instance.syntax()));
        if owner.kind(db) != hir_def::owner::OwnerKind::Module {
            return None;
        }
        let src = source_ast_id(db, file_id, instance.syntax())?;
        let module = db.body_with_source_map(owner);
        let instance_id = module.source_map().instance_srcs.src_to_hir(src)?;
        Some(OwnerRef::new(owner, instance_id))
    }

    pub fn resolve_instantiation(
        &self,
        file_id: HirFileId,
        instantiation: ast::HierarchyInstantiation,
    ) -> Option<OwnerRef<InstantiationId>> {
        let db = self.db;
        let owner = self.find_container(InFile::new(file_id, instantiation.syntax()));
        if owner.kind(db) != hir_def::owner::OwnerKind::Module {
            return None;
        }
        let src = source_ast_id(db, file_id, instantiation.syntax())?;
        let module = db.body_with_source_map(owner);
        let instantiation_id = module.source_map().instantiation_srcs.src_to_hir(src)?;
        Some(OwnerRef::new(owner, instantiation_id))
    }

    pub fn resolve_port_connection(
        &self,
        file_id: HirFileId,
        conn: ast::PortConnection,
    ) -> Option<OwnerRef<PortConnId>> {
        let db = self.db;
        let owner = self.find_container(InFile::new(file_id, conn.syntax()));
        if owner.kind(db) != hir_def::owner::OwnerKind::Module {
            return None;
        }
        let src = source_ast_id(db, file_id, conn.syntax())?;
        let module = db.body_with_source_map(owner);
        let conn_id = module.source_map().inst_port_conn_srcs.src_to_hir(src)?;
        Some(OwnerRef::new(owner, conn_id))
    }

    pub fn resolve_expr(
        &self,
        file_id: HirFileId,
        expr: ast::Expression,
    ) -> Option<OwnerRef<ExprId>> {
        let db = self.db;
        let container_id = self.find_container(InFile::new(file_id, expr.syntax()));
        let src_map = container_id.source_map(db);

        let expr_src = source_ast_id(db, file_id, expr.syntax())?;
        let expr_id = src_map.expr_from_source(expr_src)?;
        Some(OwnerRef::new(container_id, expr_id))
    }
}
