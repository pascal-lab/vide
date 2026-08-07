use hir_def::{
    ast_id_map::SourceAstId,
    container::{InContainer, InFile, InModule},
    expr::ExprId,
    module::{
        ModuleId,
        instantiation::{InstanceId, InstantiationId, PortConnId},
    },
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
    ) -> Option<InModule<InstanceId>> {
        let db = self.db;
        let owner = self.find_container(InFile::new(file_id, instance.syntax()));
        let module_id = ModuleId::from_owner(db, owner)?;

        let src = source_ast_id(db, file_id, instance.syntax())?;
        let module = db.module_with_source_map(module_id);
        let instance_id = module.source_map().instance_srcs.src_to_hir(src)?;
        Some(InModule::new(module_id, instance_id))
    }

    pub fn resolve_instantiation(
        &self,
        file_id: HirFileId,
        instantiation: ast::HierarchyInstantiation,
    ) -> Option<InModule<InstantiationId>> {
        let db = self.db;
        let owner = self.find_container(InFile::new(file_id, instantiation.syntax()));
        let module_id = ModuleId::from_owner(db, owner)?;

        let src = source_ast_id(db, file_id, instantiation.syntax())?;
        let module = db.module_with_source_map(module_id);
        let instantiation_id = module.source_map().instantiation_srcs.src_to_hir(src)?;
        Some(InModule::new(module_id, instantiation_id))
    }

    pub fn resolve_port_connection(
        &self,
        file_id: HirFileId,
        conn: ast::PortConnection,
    ) -> Option<InModule<PortConnId>> {
        let db = self.db;
        let owner = self.find_container(InFile::new(file_id, conn.syntax()));
        let module_id = ModuleId::from_owner(db, owner)?;

        let src = source_ast_id(db, file_id, conn.syntax())?;
        let module = db.module_with_source_map(module_id);
        let conn_id = module.source_map().inst_port_conn_srcs.src_to_hir(src)?;
        Some(InModule::new(module_id, conn_id))
    }

    pub fn resolve_expr(
        &self,
        file_id: HirFileId,
        expr: ast::Expression,
    ) -> Option<InContainer<ExprId>> {
        let db = self.db;
        let container_id = self.find_container(InFile::new(file_id, expr.syntax()));
        let src_map = container_id.source_map(db);

        let expr_src = source_ast_id(db, file_id, expr.syntax())?;
        let expr_id = src_map.expr_from_source(expr_src)?;
        Some(InContainer::new(container_id, expr_id))
    }
}
