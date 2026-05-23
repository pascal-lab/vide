use syntax::ast::{self, AstNode};
use utils::get::Get;

use super::SemanticsImpl;
use crate::{
    container::{ContainerId, InContainer, InFile, InModule},
    file::HirFileId,
    hir_def::{
        expr::{ExprId, ExprSrc},
        module::instantiation::{
            InstanceId, InstanceSrc, InstantiationId, InstantiationSrc, PortConnId, PortConnSrc,
        },
    },
};

impl SemanticsImpl<'_> {
    pub fn resolve_instance(
        &self,
        file_id: HirFileId,
        instance: ast::HierarchicalInstance,
    ) -> Option<InModule<InstanceId>> {
        let db = self.db;
        let ContainerId::ModuleId(module_id) =
            self.find_container(InFile::new(file_id, instance.syntax()))
        else {
            return None;
        };

        let src = InstanceSrc::from(instance);
        let (_, module_src_map) = db.module_with_source_map(module_id);
        let instance_id = module_src_map.get(src)?;
        Some(InModule::new(module_id, instance_id))
    }

    pub fn resolve_instantiation(
        &self,
        file_id: HirFileId,
        instantiation: ast::HierarchyInstantiation,
    ) -> Option<InModule<InstantiationId>> {
        let db = self.db;
        let ContainerId::ModuleId(module_id) =
            self.find_container(InFile::new(file_id, instantiation.syntax()))
        else {
            return None;
        };

        let src = InstantiationSrc::from(instantiation);
        let (_, module_src_map) = db.module_with_source_map(module_id);
        let instantiation_id = module_src_map.get(src)?;
        Some(InModule::new(module_id, instantiation_id))
    }

    pub fn resolve_port_connection(
        &self,
        file_id: HirFileId,
        conn: ast::PortConnection,
    ) -> Option<InModule<PortConnId>> {
        let db = self.db;
        let ContainerId::ModuleId(module_id) =
            self.find_container(InFile::new(file_id, conn.syntax()))
        else {
            return None;
        };

        let src = PortConnSrc::from(conn);
        let (_, module_src_map) = db.module_with_source_map(module_id);
        let conn_id = module_src_map.get(src)?;
        Some(InModule::new(module_id, conn_id))
    }

    pub fn resolve_expr(
        &self,
        file_id: HirFileId,
        expr: ast::Expression,
    ) -> Option<InContainer<ExprId>> {
        let db = self.db;
        let container_id = self.find_container(InFile::new(file_id, expr.syntax()));
        let src_map = container_id.to_container_src_map(db);

        let expr_src = ExprSrc::from(expr);
        let expr_id = src_map.get(expr_src)?;
        Some(InContainer::new(container_id, expr_id))
    }
}
