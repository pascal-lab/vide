use syntax::ast::{self, AstNode};
use utils::get::Get;

use super::SemanticsImpl;
use crate::{
    container::{ContainerId, InContainer, InFile, InModule},
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
        instance: ast::HierarchicalInstance,
    ) -> Option<InModule<InstanceId>> {
        let db = self.db;
        let file_id = self.find_file(instance.syntax())?;
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
        instantiation: ast::HierarchyInstantiation,
    ) -> Option<InModule<InstantiationId>> {
        let db = self.db;
        let file_id = self.find_file(instantiation.syntax())?;
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

    pub fn resolve_named_port_conn(
        &self,
        conn: ast::PortConnection,
    ) -> Option<InModule<PortConnId>> {
        let db = self.db;
        let file_id = self.find_file(conn.syntax())?;
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

    pub fn resolve_expr(&self, expr: ast::Expression) -> Option<InContainer<ExprId>> {
        let db = self.db;
        let file_id = self.find_file(expr.syntax())?;
        let container_id = self.find_container(InFile::new(file_id, expr.syntax()));
        let src_map = container_id.to_container_src_map(db);

        let expr_src = ExprSrc::from(expr);
        let expr_id = src_map.get(expr_src)?;
        Some(InContainer::new(container_id, expr_id))
    }
}
