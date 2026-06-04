use rustc_hash::{FxHashMap, FxHashSet};
use utils::get::GetRef;

use super::{Source2DefCtx, pathres::PathResolution};
use crate::{
    container::{
        ContainerId, ContainerParent, InBlock, InContainer, InGenerateBlock, InModule, InSubroutine,
    },
    hir_def::{
        Ident,
        block::BlockId,
        expr::{Expr, ExprId},
        module::{ModuleId, generate::GenerateBlockId, instantiation::InstanceId},
        package_import::{PackageExportName, PackageImport, PackageImportName},
    },
    type_infer::{Ty, type_of_path_resolution},
};

#[derive(Default, Debug)]
pub(super) struct Hir2DefCache {
    expr_map: FxHashMap<InContainer<ExprId>, PathResolution>,
    name_map: FxHashMap<InContainer<Ident>, PathResolution>,
}

impl Source2DefCtx<'_, '_> {
    pub(super) fn expr_to_def(
        &mut self,
        InContainer { cont_id, value: expr_id }: InContainer<ExprId>,
    ) -> Option<PathResolution> {
        let db = self.db;

        let mut resolve = |expr: &Expr| match expr {
            Expr::Field { receiver, field } => {
                let field = field.as_ref()?;
                let receiver_res = self.expr_to_def(InContainer::new(cont_id, *receiver))?;
                let res = self.resolve_member_from_resolution(receiver_res, field)?;
                self.hir_cache.expr_map.insert(InContainer::new(cont_id, expr_id), res);
                Some(res)
            }
            Expr::ElementSelect { receiver, .. } => {
                let res = self.expr_to_def(InContainer::new(cont_id, *receiver))?;
                self.hir_cache.expr_map.insert(InContainer::new(cont_id, expr_id), res);
                Some(res)
            }
            Expr::Ident(ident) => {
                let res = self.name_to_def(InContainer::new(cont_id, ident.clone()))?;
                self.hir_cache.expr_map.insert(InContainer::new(cont_id, expr_id), res);
                Some(res)
            }
            _ => None,
        };

        match cont_id {
            ContainerId::HirFileId(file_id) => {
                let file = db.hir_file(file_id);
                resolve(file.get(expr_id))
            }
            ContainerId::ModuleId(in_file) => {
                let module = db.module(in_file);
                resolve(module.get(expr_id))
            }
            ContainerId::BlockId(block_id) => {
                let block = db.block(block_id);
                resolve(block.get(expr_id))
            }
            ContainerId::GenerateBlockId(generate_block_id) => {
                let generate_block = db.generate_block(generate_block_id);
                resolve(generate_block.get(expr_id))
            }
            ContainerId::SubroutineId(subroutine_id) => {
                let subroutine = db.subroutine(subroutine_id);
                resolve(subroutine.get(expr_id))
            }
        }
    }

    pub(super) fn name_to_def(
        &mut self,
        InContainer { cont_id, value: ident }: InContainer<Ident>,
    ) -> Option<PathResolution> {
        let res = ContainerParent::start_from(self.db, cont_id).find_map(|id| {
            self.resolve_local_name(id, &ident).or_else(|| self.resolve_imported_name(id, &ident))
        })?;
        self.hir_cache.name_map.insert(InContainer::new(cont_id, ident), res);
        Some(res)
    }

    fn resolve_local_name(
        &mut self,
        cont_id: ContainerId,
        ident: &Ident,
    ) -> Option<PathResolution> {
        let db = self.db;
        match cont_id {
            ContainerId::HirFileId(_) => {
                let scope = db.unit_scope();
                let entry = scope.get(ident)?;
                Some(entry.into())
            }
            ContainerId::ModuleId(module_id) => {
                let scope = db.module_scope(module_id);
                let entry = scope.get(ident)?;
                Some(InModule::new(module_id, entry).into())
            }
            ContainerId::BlockId(block_id) => {
                let scope = db.block_scope(block_id);
                let entry = scope.get(ident)?;
                Some(InBlock::new(block_id, entry).into())
            }
            ContainerId::GenerateBlockId(generate_block_id) => {
                let scope = db.generate_block_scope(generate_block_id);
                let entry = scope.get(ident)?;
                Some(InGenerateBlock::new(generate_block_id, entry).into())
            }
            ContainerId::SubroutineId(subroutine_id) => {
                let scope = db.subroutine_scope(subroutine_id);
                let entry = scope.get(ident)?;
                Some(InSubroutine::new(subroutine_id, entry).into())
            }
        }
    }

    fn resolve_imported_name(
        &mut self,
        cont_id: ContainerId,
        ident: &Ident,
    ) -> Option<PathResolution> {
        match cont_id {
            ContainerId::HirFileId(file_id) => {
                let file = self.db.hir_file(file_id);
                self.resolve_imports(file.package_imports.iter().map(|(_, import)| import), ident)
            }
            ContainerId::ModuleId(module_id) => {
                let module = self.db.module(module_id);
                self.resolve_imports(module.package_imports.iter().map(|(_, import)| import), ident)
            }
            ContainerId::GenerateBlockId(generate_block_id) => {
                let generate_block = self.db.generate_block(generate_block_id);
                self.resolve_imports(
                    generate_block.package_imports.iter().map(|(_, import)| import),
                    ident,
                )
            }
            ContainerId::BlockId(block_id) => {
                let block = self.db.block(block_id);
                self.resolve_imports(block.package_imports.iter().map(|(_, import)| import), ident)
            }
            ContainerId::SubroutineId(subroutine_id) => {
                let subroutine = self.db.subroutine(subroutine_id);
                self.resolve_imports(
                    subroutine.package_imports.iter().map(|(_, import)| import),
                    ident,
                )
            }
        }
    }

    fn resolve_imports<'a>(
        &mut self,
        imports: impl Iterator<Item = &'a PackageImport>,
        ident: &Ident,
    ) -> Option<PathResolution> {
        let mut explicit = Vec::new();
        let mut wildcard = Vec::new();

        for import in imports {
            match &import.item {
                PackageImportName::Name(name) if name == ident => {
                    if let Some(res) = self.resolve_package_member(import.package.as_ref()?, ident)
                    {
                        push_unique_resolution(&mut explicit, res);
                    }
                }
                PackageImportName::Wildcard => {
                    if let Some(res) = self.resolve_package_member(import.package.as_ref()?, ident)
                    {
                        push_unique_resolution(&mut wildcard, res);
                    }
                }
                PackageImportName::Name(_) => {}
            }
        }

        if explicit.is_empty() { single_resolution(wildcard) } else { single_resolution(explicit) }
    }

    fn resolve_package_member(&mut self, package: &Ident, ident: &Ident) -> Option<PathResolution> {
        let package_id = self.db.unit_scope().resolve_module(package).unique()?;
        self.resolve_package_member_by_id(package_id, ident, &mut FxHashSet::default())
    }

    fn resolve_package_member_by_id(
        &mut self,
        package_id: ModuleId,
        ident: &Ident,
        seen: &mut FxHashSet<(ModuleId, Ident)>,
    ) -> Option<PathResolution> {
        if let Some(res) = self.resolve_local_member_in_module(package_id, ident) {
            return Some(res);
        }

        if !seen.insert((package_id, ident.clone())) {
            return None;
        }

        let module = self.db.module(package_id);
        let mut explicit = Vec::new();
        let mut wildcard = Vec::new();

        for (_, export) in module.package_exports.iter() {
            match &export.item {
                PackageExportName::Name(name) if name == ident => {
                    if let Some(package) = export.package.as_ref()
                        && let Some(target) = self.db.unit_scope().resolve_module(package).unique()
                        && let Some(res) = self.resolve_package_member_by_id(target, ident, seen)
                    {
                        push_unique_resolution(&mut explicit, res);
                    }
                }
                PackageExportName::Wildcard => {
                    if let Some(package) = export.package.as_ref()
                        && let Some(target) = self.db.unit_scope().resolve_module(package).unique()
                        && let Some(res) = self.resolve_package_member_by_id(target, ident, seen)
                    {
                        push_unique_resolution(&mut wildcard, res);
                    }
                }
                PackageExportName::AllImports => {
                    for (_, import) in module.package_imports.iter() {
                        match &import.item {
                            PackageImportName::Name(name) if name == ident => {
                                if let Some(package) = import.package.as_ref()
                                    && let Some(target) =
                                        self.db.unit_scope().resolve_module(package).unique()
                                    && let Some(res) =
                                        self.resolve_package_member_by_id(target, ident, seen)
                                {
                                    push_unique_resolution(&mut explicit, res);
                                }
                            }
                            PackageImportName::Wildcard => {
                                if let Some(package) = import.package.as_ref()
                                    && let Some(target) =
                                        self.db.unit_scope().resolve_module(package).unique()
                                    && let Some(res) =
                                        self.resolve_package_member_by_id(target, ident, seen)
                                {
                                    push_unique_resolution(&mut wildcard, res);
                                }
                            }
                            PackageImportName::Name(_) => {}
                        }
                    }
                }
                PackageExportName::Name(_) => {}
            }
        }

        seen.remove(&(package_id, ident.clone()));
        if explicit.is_empty() { single_resolution(wildcard) } else { single_resolution(explicit) }
    }

    fn resolve_member_from_resolution(
        &mut self,
        res: PathResolution,
        field: &Ident,
    ) -> Option<PathResolution> {
        match res {
            PathResolution::Module(module_id) => self.resolve_member_in_module(module_id, field),
            PathResolution::Instance(instance) => {
                let target_module =
                    self.instance_target_module_id(instance.module_id, instance.value)?;
                self.resolve_member_in_module(target_module, field)
            }
            PathResolution::Block(block_id) => self.resolve_member_in_block(block_id, field),
            PathResolution::GenerateBlock(generate_block_id) => {
                self.resolve_member_in_generate_block(generate_block_id, field)
            }
            _ => self.resolve_member_from_ty(&type_of_path_resolution(self.db, res).ty, field),
        }
    }

    fn resolve_member_from_ty(&mut self, ty: &Ty, field: &Ident) -> Option<PathResolution> {
        match ty {
            Ty::Module(module_id) => self.resolve_member_in_module(*module_id, field),
            Ty::GenerateBlock(generate_block_id) => {
                self.resolve_member_in_generate_block(*generate_block_id, field)
            }
            Ty::Block(block_id) => self.resolve_member_in_block(*block_id, field),
            Ty::Alias { target, .. } => self.resolve_member_from_ty(target, field),
            Ty::Unknown | Ty::Error | Ty::Void | Ty::Builtin(_) | Ty::Struct(_) => None,
        }
    }

    fn resolve_member_in_module(
        &mut self,
        module_id: ModuleId,
        field: &Ident,
    ) -> Option<PathResolution> {
        self.resolve_package_member_by_id(module_id, field, &mut FxHashSet::default())
    }

    fn resolve_local_member_in_module(
        &mut self,
        module_id: ModuleId,
        field: &Ident,
    ) -> Option<PathResolution> {
        let scope = self.db.module_scope(module_id);
        let entry = scope.get(field)?;
        Some(InModule::new(module_id, entry).into())
    }

    fn resolve_member_in_block(
        &mut self,
        block_id: BlockId,
        field: &Ident,
    ) -> Option<PathResolution> {
        let scope = self.db.block_scope(block_id);
        let entry = scope.get(field)?;
        Some(InBlock::new(block_id, entry).into())
    }

    fn resolve_member_in_generate_block(
        &mut self,
        generate_block_id: GenerateBlockId,
        field: &Ident,
    ) -> Option<PathResolution> {
        let scope = self.db.generate_block_scope(generate_block_id);
        let entry = scope.get(field)?;
        Some(InGenerateBlock::new(generate_block_id, entry).into())
    }

    fn instance_target_module_id(
        &mut self,
        module_id: ModuleId,
        instance_id: InstanceId,
    ) -> Option<ModuleId> {
        let module = self.db.module(module_id);
        let instance = module.get(instance_id);
        let instantiation = module.get(instance.parent);
        let module_name = instantiation.module_name.as_ref()?;
        self.db.unit_scope().resolve_module(module_name).unique()
    }
}

fn push_unique_resolution(resolutions: &mut Vec<PathResolution>, res: PathResolution) {
    if !resolutions.contains(&res) {
        resolutions.push(res);
    }
}

fn single_resolution(mut resolutions: Vec<PathResolution>) -> Option<PathResolution> {
    if resolutions.len() == 1 { resolutions.pop() } else { None }
}
