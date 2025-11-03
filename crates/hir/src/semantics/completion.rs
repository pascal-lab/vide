use itertools::Either;
use rustc_hash::FxHashSet;
use smol_str::SmolStr;
use syntax::{SyntaxNodeExt, ast::AstNode};
use utils::{get::GetRef, text_edit::TextSize};
use vfs::FileId;

use super::SemanticsImpl;
use crate::{
    completion::{
        CompletionEntry, CompletionEntryKind, CompletionScope, DotField, DotFieldKind,
        ScopedCompletionEntry,
    },
    container::{ContainerId, ContainerParent, InContainer, InFile, InModule},
    display::HirDisplay,
    file::HirFileId,
    hir_def::{
        Ident,
        aggregate::{ClassDef, ClassId, ClassMemberKind, StructDef, StructId},
        expr::{
            data_ty::{DataTy, NamedDataTy},
            declarator::{DeclId, DeclaratorParent},
        },
        module::ModuleId,
        package::{PackageId, PackageImportMember},
        stmt::{ForInit, Stmt, StmtId, StmtKind},
        typedef::TypedefId,
    },
    scope::{ModuleEntry, PackageEntry, PackageImportEntry, UnitEntry},
    semantics::PathResolution,
};

#[derive(Debug, Clone)]
enum ScopeResolvedType {
    Struct(InContainer<StructId>),
    Class(InContainer<ClassId>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ScopeResolutionTarget {
    Package(PackageId),
    Module(ModuleId),
    Class(InContainer<ClassId>),
}

impl<'db> SemanticsImpl<'db> {
    pub fn scope_completions(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<ScopedCompletionEntry> {
        let hir_file_id = HirFileId::from(file_id);
        let root = self.parse(file_id);
        let syntax = root.syntax();

        let node = match syntax.token_or_node_at_offset(offset) {
            Either::Left(tok_at_offset) => {
                tok_at_offset.left_biased().map(|tok| tok.parent).unwrap_or_else(|| syntax)
            }
            Either::Right(node) => node,
        };

        let container_id = self.with_ctx(|ctx| ctx.find_container(InFile::new(hir_file_id, node)));
        let mut seen = FxHashSet::default();
        let mut items = Vec::new();

        for cont_id in ContainerParent::start_from(self.db, container_id) {
            match cont_id {
                ContainerId::BlockId(block_id) => {
                    let scope = self.db.block_scope(block_id);
                    for entry in scope.collect_completions(self.db, block_id) {
                        if seen.insert(entry.name.clone()) {
                            items.push(ScopedCompletionEntry {
                                entry,
                                scope: CompletionScope::Local,
                            });
                        }
                    }
                }
                ContainerId::ModuleId(module_id) => {
                    let scope = self.db.module_scope(module_id);
                    for entry in scope.collect_completions(self.db, module_id) {
                        if seen.insert(entry.name.clone()) {
                            items.push(ScopedCompletionEntry {
                                entry,
                                scope: CompletionScope::Module,
                            });
                        }
                    }
                }
                ContainerId::PackageId(package_id) => {
                    let scope = self.db.package_scope(package_id);
                    for entry in scope.collect_completions(self.db, package_id) {
                        if seen.insert(entry.name.clone()) {
                            items.push(ScopedCompletionEntry {
                                entry,
                                scope: CompletionScope::Package,
                            });
                        }
                    }
                }
                ContainerId::SubroutineId(loc) => {
                    let scope = self.db.subroutine_scope(loc);
                    for entry in scope.collect_completions(self.db, loc) {
                        if seen.insert(entry.name.clone()) {
                            items.push(ScopedCompletionEntry {
                                entry,
                                scope: CompletionScope::Subroutine,
                            });
                        }
                    }
                }
                ContainerId::FileSubroutineId(loc) => {
                    let scope = self.db.file_subroutine_scope(loc);
                    for entry in scope.collect_file_subroutine_completions(self.db, loc) {
                        if seen.insert(entry.name.clone()) {
                            items.push(ScopedCompletionEntry {
                                entry,
                                scope: CompletionScope::Subroutine,
                            });
                        }
                    }
                }
                ContainerId::HirFileId(file_id) => {
                    let scope = self.db.file_scope(file_id);
                    for entry in scope.collect_completions(self.db) {
                        if seen.insert(entry.name.clone()) {
                            items.push(ScopedCompletionEntry {
                                entry,
                                scope: CompletionScope::File,
                            });
                        }
                    }
                }
            }
        }

        let unit_scope = self.db.unit_scope();
        for entry in unit_scope.collect_completions(self.db) {
            if seen.insert(entry.name.clone()) {
                items.push(ScopedCompletionEntry { entry, scope: CompletionScope::Unit });
            }
        }

        items
    }

    pub fn scope_resolution_completions(
        &self,
        _file_id: FileId,
        chain: &[SmolStr],
        prefix: &str,
    ) -> Vec<ScopedCompletionEntry> {
        if chain.is_empty() {
            return Vec::new();
        }

        let unit_scope = self.db.unit_scope();
        let packages_by_name = self.db.packages_by_name();

        let first_ident = Ident::from(chain[0].clone());
        let mut targets_set = FxHashSet::default();

        if let Some(entry) = unit_scope.get(&first_ident)
            && let Some(target) = self.unit_entry_to_scope_target(entry)
        {
            targets_set.insert(target);
        }

        if let Some(pkg_ids) = packages_by_name.get(&first_ident) {
            for pkg_id in pkg_ids.iter().copied() {
                targets_set.insert(ScopeResolutionTarget::Package(pkg_id));
            }
        }

        let mut targets: Vec<_> = targets_set.into_iter().collect();
        if targets.is_empty() {
            return Vec::new();
        }

        for segment in chain.iter().skip(1) {
            let ident = Ident::from(segment.clone());
            let mut next_targets_set = FxHashSet::default();

            for target in targets.iter().copied() {
                match target {
                    ScopeResolutionTarget::Package(package_id) => {
                        let package_scope = self.db.package_scope(package_id);
                        let mut matched = false;

                        if let Some(entry) = package_scope.get(&ident) {
                            matched = true;
                            if let Some(next) = self.package_entry_to_scope_target(entry) {
                                next_targets_set.insert(next);
                            }
                        }

                        if !matched && let Some(pkg_ids) = packages_by_name.get(&ident) {
                            for pkg_id in pkg_ids.iter().copied() {
                                next_targets_set.insert(ScopeResolutionTarget::Package(pkg_id));
                            }
                        }
                    }
                    ScopeResolutionTarget::Module(module_id) => {
                        let module_scope = self.db.module_scope(module_id);
                        if let Some(entry) = module_scope.get(&ident)
                            && let Some(next) = self.module_entry_to_scope_target(module_id, entry)
                        {
                            next_targets_set.insert(next);
                        }
                    }
                    ScopeResolutionTarget::Class(class_ref) => {
                        let mut handle_class = |def: &ClassDef| {
                            for member in &def.members {
                                if member.kind == ClassMemberKind::Typedef
                                    && member.name.as_ref().map_or(false, |n| n == &ident)
                                {
                                    if let Some(ty) = member.ty {
                                        if let Some(resolved) = self.data_ty_to_scope_type(ty) {
                                            if let ScopeResolvedType::Class(nested_class) = resolved
                                            {
                                                next_targets_set.insert(
                                                    ScopeResolutionTarget::Class(nested_class),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        };

                        match class_ref.cont_id {
                            ContainerId::HirFileId(file_id) => {
                                let file = self.db.hir_file(file_id);
                                handle_class(file.classes.get(class_ref.value));
                            }
                            ContainerId::ModuleId(module_id) => {
                                let module = self.db.module(module_id);
                                handle_class(module.classes.get(class_ref.value));
                            }
                            ContainerId::PackageId(package_id) => {
                                let package = self.db.package(package_id);
                                handle_class(package.classes.get(class_ref.value));
                            }
                            _ => {}
                        }
                    }
                }
            }

            if next_targets_set.is_empty() {
                return Vec::new();
            }

            targets = next_targets_set.into_iter().collect();
        }

        let mut seen = FxHashSet::default();
        let mut results = Vec::new();

        for target in targets {
            let (entries, scope) = match target {
                ScopeResolutionTarget::Package(package_id) => {
                    let scope = self.db.package_scope(package_id);
                    let entries = scope.collect_completions(self.db, package_id);
                    (entries, CompletionScope::Package)
                }
                ScopeResolutionTarget::Module(module_id) => {
                    let scope = self.db.module_scope(module_id);
                    let entries = scope.collect_completions(self.db, module_id);
                    (entries, CompletionScope::Module)
                }
                ScopeResolutionTarget::Class(class_ref) => {
                    let entries = self.class_scope_completions(class_ref, prefix);
                    (entries, CompletionScope::Class)
                }
            };

            for entry in entries {
                if seen.insert(entry.name.clone()) {
                    results.push(ScopedCompletionEntry { entry, scope });
                }
            }
        }

        results
    }

    fn unit_entry_to_scope_target(&self, entry: UnitEntry) -> Option<ScopeResolutionTarget> {
        match entry {
            UnitEntry::PackageId(package_id) => Some(ScopeResolutionTarget::Package(package_id)),
            UnitEntry::ModuleId(module_id) => Some(ScopeResolutionTarget::Module(module_id)),
            UnitEntry::ClassId(class_id) => Some(ScopeResolutionTarget::Class(class_id.into())),
            UnitEntry::FiledDeclId(_) => None,
            UnitEntry::TypedefId(_) => None,
        }
    }

    fn package_entry_to_scope_target(&self, entry: PackageEntry) -> Option<ScopeResolutionTarget> {
        match entry {
            PackageEntry::ClassId(in_pkg_class) => {
                Some(ScopeResolutionTarget::Class(in_pkg_class.into()))
            }
            PackageEntry::StructId(_) => None,
            PackageEntry::DeclId(_) => None,
            PackageEntry::TypedefId(_) => None,
            PackageEntry::ProcId(_) => None,
            PackageEntry::SubroutineId(_) => None,
            PackageEntry::Package(in_pkg_pkg) => {
                Some(ScopeResolutionTarget::Package(in_pkg_pkg.value))
            }
        }
    }

    fn module_entry_to_scope_target(
        &self,
        module_id: ModuleId,
        entry: ModuleEntry,
    ) -> Option<ScopeResolutionTarget> {
        match entry {
            ModuleEntry::ClassId(class_id) => Some(ScopeResolutionTarget::Class(InContainer::new(
                ContainerId::ModuleId(module_id),
                class_id,
            ))),
            ModuleEntry::PackageMember(pkg_entry) => self.package_entry_to_scope_target(pkg_entry),
            ModuleEntry::DeclId(_)
            | ModuleEntry::NonAnsiPortEntry(_)
            | ModuleEntry::AnsiPortEntry(_)
            | ModuleEntry::InstanceId(_)
            | ModuleEntry::StmtId(_)
            | ModuleEntry::BlockId(_)
            | ModuleEntry::TypedefId(_)
            | ModuleEntry::PackageImportEntry(_)
            | ModuleEntry::SubroutineId(_) => None,
        }
    }

    pub fn dot_completions(
        &self,
        file_id: FileId,
        offset: TextSize,
        chain: &[SmolStr],
        prefix: &str,
    ) -> Vec<DotField> {
        if chain.is_empty() {
            return Vec::new();
        }

        let container_id = self.container_at_offset(file_id, offset);
        let mut current_type: Option<ScopeResolvedType> = None;

        for (idx, ident) in chain.iter().enumerate() {
            if idx == 0 {
                current_type = self.resolve_identifier_type(container_id, ident);
            } else if let Some(ref ty) = current_type {
                current_type = self.resolve_field_scope_type(ty, ident);
            } else {
                return Vec::new();
            }

            if current_type.is_none() {
                return Vec::new();
            }
        }

        let Some(ty) = current_type else {
            return Vec::new();
        };
        self.collect_fields(&ty, prefix)
    }

    fn container_at_offset(&self, file_id: FileId, offset: TextSize) -> ContainerId {
        let hir_file_id = HirFileId::from(file_id);
        let root = self.parse(file_id);
        let syntax = root.syntax();

        let node = match syntax.token_or_node_at_offset(offset) {
            Either::Left(tok_at_offset) => {
                tok_at_offset.left_biased().map(|tok| tok.parent).unwrap_or_else(|| syntax)
            }
            Either::Right(node) => node,
        };

        self.with_ctx(|ctx| ctx.find_container(InFile::new(hir_file_id, node)))
    }

    fn resolve_identifier_type(
        &self,
        container_id: ContainerId,
        ident: &SmolStr,
    ) -> Option<ScopeResolvedType> {
        let resolution =
            self.with_ctx(|ctx| ctx.name_to_def(InContainer::new(container_id, ident.clone())))?;
        self.path_resolution_to_scope_type(resolution)
    }

    fn resolve_field_scope_type(
        &self,
        ty: &ScopeResolvedType,
        field_name: &SmolStr,
    ) -> Option<ScopeResolvedType> {
        match ty {
            ScopeResolvedType::Struct(struct_ref) => {
                self.struct_field_type(*struct_ref, field_name)
            }
            ScopeResolvedType::Class(class_ref) => self.class_field_type(*class_ref, field_name),
        }
    }

    fn struct_field_type(
        &self,
        struct_ref: InContainer<StructId>,
        field_name: &SmolStr,
    ) -> Option<ScopeResolvedType> {
        match struct_ref.cont_id {
            ContainerId::HirFileId(file_id) => {
                let file = self.db.hir_file(file_id);
                let def = file.structs.get(struct_ref.value);
                self.struct_member_type(def, field_name)
            }
            ContainerId::ModuleId(module_id) => {
                let module = self.db.module(module_id);
                let def = module.structs.get(struct_ref.value);
                self.struct_member_type(def, field_name)
            }
            ContainerId::PackageId(package_id) => {
                let package = self.db.package(package_id);
                let def = package.structs.get(struct_ref.value);
                self.struct_member_type(def, field_name)
            }
            ContainerId::BlockId(block_id) => {
                let block = self.db.block(block_id);
                let def = block.structs.get(struct_ref.value);
                self.struct_member_type(def, field_name)
            }
            ContainerId::SubroutineId(loc) => {
                let subroutine = self.db.subroutine(loc);
                let def = subroutine.structs.get(struct_ref.value);
                self.struct_member_type(def, field_name)
            }
            ContainerId::FileSubroutineId(loc) => {
                let subroutine = loc.to_container(self.db);
                let def = subroutine.structs.get(struct_ref.value);
                self.struct_member_type(def, field_name)
            }
        }
    }

    fn class_field_type(
        &self,
        class_ref: InContainer<ClassId>,
        field_name: &SmolStr,
    ) -> Option<ScopeResolvedType> {
        match class_ref.cont_id {
            ContainerId::HirFileId(file_id) => {
                let file = self.db.hir_file(file_id);
                let def = file.classes.get(class_ref.value);
                self.class_member_type(def, field_name)
            }
            ContainerId::ModuleId(module_id) => {
                let module = self.db.module(module_id);
                let def = module.classes.get(class_ref.value);
                self.class_member_type(def, field_name)
            }
            ContainerId::PackageId(package_id) => {
                let package = self.db.package(package_id);
                let def = package.classes.get(class_ref.value);
                self.class_member_type(def, field_name)
            }
            ContainerId::BlockId(_) => None,
            ContainerId::SubroutineId(_) => None,
            ContainerId::FileSubroutineId(_) => None,
        }
    }

    fn struct_member_type(
        &self,
        def: &StructDef,
        field_name: &SmolStr,
    ) -> Option<ScopeResolvedType> {
        let member = def.members.iter().find(|member| {
            member.name.as_ref().map(|name| name.as_str() == field_name.as_str()).unwrap_or(false)
        })?;
        let ty = member.ty?;
        self.data_ty_to_scope_type(ty)
    }

    fn class_member_type(&self, def: &ClassDef, field_name: &SmolStr) -> Option<ScopeResolvedType> {
        let member = def.members.iter().find(|member| {
            member.name.as_ref().map(|name| name.as_str() == field_name.as_str()).unwrap_or(false)
        })?;
        match member.kind {
            ClassMemberKind::Property => {
                let ty = member.ty?;
                self.data_ty_to_scope_type(ty)
            }
            _ => None,
        }
    }

    fn collect_fields(&self, ty: &ScopeResolvedType, prefix: &str) -> Vec<DotField> {
        let mut items = match ty {
            ScopeResolvedType::Struct(struct_ref) => {
                self.collect_struct_fields(*struct_ref, prefix)
            }
            ScopeResolvedType::Class(class_ref) => self.collect_class_fields(*class_ref, prefix),
        };
        items.sort_by(|lhs, rhs| lhs.name.as_str().cmp(rhs.name.as_str()));
        items
    }

    fn collect_struct_fields(
        &self,
        struct_ref: InContainer<StructId>,
        prefix: &str,
    ) -> Vec<DotField> {
        match struct_ref.cont_id {
            ContainerId::HirFileId(file_id) => {
                let file = self.db.hir_file(file_id);
                self.collect_struct_def_fields(file.structs.get(struct_ref.value), prefix)
            }
            ContainerId::ModuleId(module_id) => {
                let module = self.db.module(module_id);
                self.collect_struct_def_fields(module.structs.get(struct_ref.value), prefix)
            }
            ContainerId::PackageId(package_id) => {
                let package = self.db.package(package_id);
                self.collect_struct_def_fields(package.structs.get(struct_ref.value), prefix)
            }
            ContainerId::BlockId(block_id) => {
                let block = self.db.block(block_id);
                self.collect_struct_def_fields(block.structs.get(struct_ref.value), prefix)
            }
            ContainerId::SubroutineId(loc) => {
                let subroutine = self.db.subroutine(loc);
                self.collect_struct_def_fields(subroutine.structs.get(struct_ref.value), prefix)
            }
            ContainerId::FileSubroutineId(loc) => {
                let subroutine = loc.to_container(self.db);
                self.collect_struct_def_fields(subroutine.structs.get(struct_ref.value), prefix)
            }
        }
    }

    fn collect_class_fields(&self, class_ref: InContainer<ClassId>, prefix: &str) -> Vec<DotField> {
        match class_ref.cont_id {
            ContainerId::HirFileId(file_id) => {
                let file = self.db.hir_file(file_id);
                self.collect_class_def_fields(file.classes.get(class_ref.value), prefix)
            }
            ContainerId::ModuleId(module_id) => {
                let module = self.db.module(module_id);
                self.collect_class_def_fields(module.classes.get(class_ref.value), prefix)
            }
            ContainerId::PackageId(package_id) => {
                let package = self.db.package(package_id);
                self.collect_class_def_fields(package.classes.get(class_ref.value), prefix)
            }
            ContainerId::BlockId(_) | ContainerId::SubroutineId(_) | ContainerId::FileSubroutineId(_) => Vec::new(),
        }
    }

    fn collect_struct_def_fields(&self, def: &StructDef, prefix: &str) -> Vec<DotField> {
        let mut items = Vec::new();

        for member in &def.members {
            let Some(name) = &member.name else { continue };
            if !prefix.is_empty() && !name.as_str().starts_with(prefix) {
                continue;
            }

            let detail = member.ty.as_ref().and_then(|ty| ty.display_signature(self.db).ok());
            items.push(DotField { name: name.clone(), detail, kind: DotFieldKind::Field });
        }

        items
    }

    fn collect_class_def_fields(&self, def: &ClassDef, prefix: &str) -> Vec<DotField> {
        let mut items = Vec::new();

        for member in &def.members {
            let Some(name) = &member.name else { continue };
            if !prefix.is_empty() && !name.as_str().starts_with(prefix) {
                continue;
            }

            match member.kind {
                ClassMemberKind::Property => {
                    let detail =
                        member.ty.as_ref().and_then(|ty| ty.display_signature(self.db).ok());
                    items.push(DotField { name: name.clone(), detail, kind: DotFieldKind::Field });
                }
                ClassMemberKind::Method => {
                    items.push(DotField {
                        name: name.clone(),
                        detail: Some(String::from("method")),
                        kind: DotFieldKind::Method,
                    });
                }
                ClassMemberKind::Typedef | ClassMemberKind::Unknown => {}
            }
        }

        items
    }

    fn class_scope_completions(
        &self,
        class_ref: InContainer<ClassId>,
        prefix: &str,
    ) -> Vec<CompletionEntry> {
        match class_ref.cont_id {
            ContainerId::HirFileId(file_id) => {
                let file = self.db.hir_file(file_id);
                self.collect_class_scope_entries(file.classes.get(class_ref.value), prefix)
            }
            ContainerId::ModuleId(module_id) => {
                let module = self.db.module(module_id);
                self.collect_class_scope_entries(module.classes.get(class_ref.value), prefix)
            }
            ContainerId::PackageId(package_id) => {
                let package = self.db.package(package_id);
                self.collect_class_scope_entries(package.classes.get(class_ref.value), prefix)
            }
            ContainerId::BlockId(_) | ContainerId::SubroutineId(_) | ContainerId::FileSubroutineId(_) => Vec::new(),
        }
    }

    fn collect_class_scope_entries(&self, def: &ClassDef, prefix: &str) -> Vec<CompletionEntry> {
        let mut items = Vec::new();

        for member in &def.members {
            let Some(name) = &member.name else { continue };
            if !prefix.is_empty() && !name.as_str().starts_with(prefix) {
                continue;
            }

            let completion = match member.kind {
                ClassMemberKind::Property => {
                    let detail = member
                        .ty
                        .as_ref()
                        .and_then(|ty| ty.display_signature(self.db).ok())
                        .unwrap_or_else(|| CompletionEntryKind::Variable.as_str().to_string());
                    CompletionEntry::new(name.clone(), CompletionEntryKind::Variable)
                        .with_detail(detail)
                }
                ClassMemberKind::Method => {
                    CompletionEntry::new(name.clone(), CompletionEntryKind::Function)
                        .with_detail(CompletionEntryKind::Function.as_str())
                }
                ClassMemberKind::Typedef => {
                    CompletionEntry::new(name.clone(), CompletionEntryKind::Type)
                        .with_detail(CompletionEntryKind::Type.as_str())
                }
                ClassMemberKind::Unknown => continue,
            };

            items.push(completion);
        }

        items.sort_by(|lhs, rhs| lhs.name.cmp(&rhs.name));
        items
    }

    fn path_resolution_to_scope_type(&self, res: PathResolution) -> Option<ScopeResolvedType> {
        let ty = self.path_resolution_to_data_ty(res)?;
        self.data_ty_to_scope_type(ty)
    }

    fn path_resolution_to_data_ty(&self, res: PathResolution) -> Option<InContainer<DataTy>> {
        match res {
            PathResolution::Decl(decl) => self.decl_type(decl),
            PathResolution::ParamDecl(param_decl) => self.decl_type(param_decl.into()),
            PathResolution::Typedef(typedef) => self.typedef_type(typedef),
            PathResolution::Class(class_ref) => {
                Some(InContainer::new(class_ref.cont_id, DataTy::Class(class_ref)))
            }
            PathResolution::PackageImport(import) => self.package_import_type(import),
            _ => None,
        }
    }

    fn package_entry_to_data_ty(&self, entry: PackageEntry) -> Option<InContainer<DataTy>> {
        match entry {
            PackageEntry::DeclId(in_pkg_decl) => self.decl_type(in_pkg_decl.into()),
            PackageEntry::TypedefId(in_pkg_typedef) => self.typedef_type(in_pkg_typedef.into()),
            PackageEntry::ClassId(in_pkg_class) => {
                let in_container: InContainer<ClassId> = in_pkg_class.into();
                Some(InContainer::new(in_container.cont_id, DataTy::Class(in_container)))
            }
            PackageEntry::StructId(_)
            | PackageEntry::ProcId(_)
            | PackageEntry::SubroutineId(_)
            | PackageEntry::Package(_) => None,
        }
    }

    fn package_import_type(
        &self,
        import: InModule<PackageImportEntry>,
    ) -> Option<InContainer<DataTy>> {
        let module_id = import.module_id;
        let module = self.db.module(module_id);
        let import_entry = module.package_imports.get(import.value.import);
        let item = import_entry.items.get(import.value.item_idx as usize)?;

        match &item.member {
            PackageImportMember::Named(name) => {
                let unit_scope = self.db.unit_scope();
                let package_id = match unit_scope.get(&item.package)? {
                    UnitEntry::PackageId(package_id) => package_id,
                    _ => return None,
                };

                let package_scope = self.db.package_scope(package_id);
                let entry = package_scope.get(name)?;
                self.package_entry_to_data_ty(entry)
            }
            PackageImportMember::All => None,
        }
    }

    fn decl_type(&self, decl: InContainer<DeclId>) -> Option<InContainer<DataTy>> {
        match decl.cont_id {
            ContainerId::HirFileId(file_id) => {
                let file = self.db.hir_file(file_id);
                let declarator = file.decls.get(decl.value);
                match declarator.parent {
                    DeclaratorParent::DeclarationId(declaration_id) => {
                        let declaration = file.declarations.get(declaration_id);
                        Some(InContainer::new(decl.cont_id, declaration.ty()))
                    }
                    DeclaratorParent::PortDeclId(_) => None,
                    DeclaratorParent::StmtId(stmt_id) => {
                        self.stmt_decl_ty(decl.cont_id, stmt_id, decl.value)
                    }
                }
            }
            ContainerId::ModuleId(module_id) => {
                let module = self.db.module(module_id);
                let declarator = module.decls.get(decl.value);
                match declarator.parent {
                    DeclaratorParent::DeclarationId(declaration_id) => {
                        let declaration = module.declarations.get(declaration_id);
                        Some(InContainer::new(decl.cont_id, declaration.ty()))
                    }
                    DeclaratorParent::PortDeclId(_) => None,
                    DeclaratorParent::StmtId(stmt_id) => {
                        self.stmt_decl_ty(decl.cont_id, stmt_id, decl.value)
                    }
                }
            }
            ContainerId::PackageId(package_id) => {
                let package = self.db.package(package_id);
                let declarator = package.decls.get(decl.value);
                match declarator.parent {
                    DeclaratorParent::DeclarationId(declaration_id) => {
                        let declaration = package.declarations.get(declaration_id);
                        Some(InContainer::new(decl.cont_id, declaration.ty()))
                    }
                    DeclaratorParent::PortDeclId(_) => None,
                    DeclaratorParent::StmtId(stmt_id) => {
                        self.stmt_decl_ty(decl.cont_id, stmt_id, decl.value)
                    }
                }
            }
            ContainerId::BlockId(block_id) => {
                let block = self.db.block(block_id);
                let declarator = block.decls.get(decl.value);
                match declarator.parent {
                    DeclaratorParent::DeclarationId(declaration_id) => {
                        let declaration = block.declarations.get(declaration_id);
                        Some(InContainer::new(decl.cont_id, declaration.ty()))
                    }
                    DeclaratorParent::PortDeclId(_) => None,
                    DeclaratorParent::StmtId(stmt_id) => {
                        self.stmt_decl_ty(decl.cont_id, stmt_id, decl.value)
                    }
                }
            }
            ContainerId::SubroutineId(loc) => {
                let subroutine = self.db.subroutine(loc);
                let declarator = subroutine.decls.get(decl.value);
                match declarator.parent {
                    DeclaratorParent::DeclarationId(declaration_id) => {
                        let declaration = subroutine.declarations.get(declaration_id);
                        Some(InContainer::new(decl.cont_id, declaration.ty()))
                    }
                    DeclaratorParent::PortDeclId(_) => None,
                    DeclaratorParent::StmtId(stmt_id) => {
                        self.stmt_decl_ty(decl.cont_id, stmt_id, decl.value)
                    }
                }
            }
            ContainerId::FileSubroutineId(loc) => {
                let subroutine = loc.to_container(self.db);
                let declarator = subroutine.decls.get(decl.value);
                match declarator.parent {
                    DeclaratorParent::DeclarationId(declaration_id) => {
                        let declaration = subroutine.declarations.get(declaration_id);
                        Some(InContainer::new(decl.cont_id, declaration.ty()))
                    }
                    DeclaratorParent::PortDeclId(_) => None,
                    DeclaratorParent::StmtId(stmt_id) => {
                        self.stmt_decl_ty(decl.cont_id, stmt_id, decl.value)
                    }
                }
            }
        }
    }

    fn typedef_type(&self, typedef: InContainer<TypedefId>) -> Option<InContainer<DataTy>> {
        match typedef.cont_id {
            ContainerId::HirFileId(file_id) => {
                let file = self.db.hir_file(file_id);
                file.typedefs.get(typedef.value).ty.map(|ty| InContainer::new(typedef.cont_id, ty))
            }
            ContainerId::ModuleId(module_id) => {
                let module = self.db.module(module_id);
                module
                    .typedefs
                    .get(typedef.value)
                    .ty
                    .map(|ty| InContainer::new(typedef.cont_id, ty))
            }
            ContainerId::PackageId(package_id) => {
                let package = self.db.package(package_id);
                package
                    .typedefs
                    .get(typedef.value)
                    .ty
                    .map(|ty| InContainer::new(typedef.cont_id, ty))
            }
            ContainerId::BlockId(block_id) => {
                let block = self.db.block(block_id);
                block.typedefs.get(typedef.value).ty.map(|ty| InContainer::new(typedef.cont_id, ty))
            }
            ContainerId::SubroutineId(loc) => {
                let subroutine = self.db.subroutine(loc);
                subroutine
                    .typedefs
                    .get(typedef.value)
                    .ty
                    .map(|ty| InContainer::new(typedef.cont_id, ty))
            }
            ContainerId::FileSubroutineId(loc) => {
                let subroutine = loc.to_container(self.db);
                subroutine
                    .typedefs
                    .get(typedef.value)
                    .ty
                    .map(|ty| InContainer::new(typedef.cont_id, ty))
            }
        }
    }

    fn data_ty_to_scope_type(&self, ty: InContainer<DataTy>) -> Option<ScopeResolvedType> {
        match ty.value {
            DataTy::Struct(struct_ref) => Some(ScopeResolvedType::Struct(struct_ref)),
            DataTy::Class(class_ref) => Some(ScopeResolvedType::Class(class_ref)),
            DataTy::Named(named) => {
                let resolved = self.resolve_named_data_ty(ty.with_value(named))?;
                self.data_ty_to_scope_type(resolved)
            }
            DataTy::Builtin(_) => None,
        }
    }

    fn resolve_named_data_ty(
        &self,
        named: InContainer<NamedDataTy>,
    ) -> Option<InContainer<DataTy>> {
        let expr_id = match named.value {
            NamedDataTy::Ident(expr_id) | NamedDataTy::Field(expr_id) => expr_id,
        };
        let resolution = self.expr_to_def(named.with_value(expr_id))?;
        self.path_resolution_to_data_ty(resolution)
    }

    fn stmt_decl_ty(
        &self,
        cont_id: ContainerId,
        stmt_id: StmtId,
        decl_id: DeclId,
    ) -> Option<InContainer<DataTy>> {
        match cont_id {
            ContainerId::HirFileId(file_id) => {
                let file = self.db.hir_file(file_id);
                let stmt = file.stmts.get(stmt_id);
                self.stmt_decl_ty_from_stmt(cont_id, stmt, decl_id)
            }
            ContainerId::ModuleId(module_id) => {
                let module = self.db.module(module_id);
                let stmt = module.stmts.get(stmt_id);
                self.stmt_decl_ty_from_stmt(cont_id, stmt, decl_id)
            }
            ContainerId::PackageId(package_id) => {
                let package = self.db.package(package_id);
                let stmt = package.stmts.get(stmt_id);
                self.stmt_decl_ty_from_stmt(cont_id, stmt, decl_id)
            }
            ContainerId::BlockId(block_id) => {
                let block = self.db.block(block_id);
                let stmt = block.stmts.get(stmt_id);
                self.stmt_decl_ty_from_stmt(cont_id, stmt, decl_id)
            }
            ContainerId::SubroutineId(loc) => {
                let subroutine = self.db.subroutine(loc);
                let stmt = subroutine.stmts.get(stmt_id);
                self.stmt_decl_ty_from_stmt(cont_id, stmt, decl_id)
            }
            ContainerId::FileSubroutineId(loc) => {
                let subroutine = loc.to_container(self.db);
                let stmt = subroutine.stmts.get(stmt_id);
                self.stmt_decl_ty_from_stmt(cont_id, stmt, decl_id)
            }
        }
    }

    fn stmt_decl_ty_from_stmt(
        &self,
        cont_id: ContainerId,
        stmt: &Stmt,
        decl_id: DeclId,
    ) -> Option<InContainer<DataTy>> {
        match &stmt.kind {
            StmtKind::For { inits: ForInit::Init(inits), .. } => inits
                .iter()
                .find(|(_, id)| *id == decl_id)
                .map(|(ty, _)| InContainer::new(cont_id, *ty)),
            _ => None,
        }
    }
}
