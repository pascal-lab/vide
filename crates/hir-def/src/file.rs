use config::{ConfigDecl, ConfigDeclId};
use library::{LibraryDecl, LibraryDeclId, LibraryInclude, LibraryIncludeId};
pub use preproc_expand::file::HirFileId;
use syntax::ast::{self, AstNode};
use triomphe::Arc;
use udp::{UdpDecl, UdpDeclId};

use super::{
    aggregate::{StructId, lower_struct_def},
    alloc_with_source,
    covergroup::{CovergroupId, lower_covergroup_decl, lower_coverpoint, lower_cross},
    lower::{FileStore, LoweringCtx, LoweringSyntax},
    lower_package_imports,
    module::{ModuleInfo, ModuleKind},
    subroutine::{LocalSubroutineId, lower_subroutine},
    typedef::{Typedef, TypedefId, lower_typedef_data_ty},
};
use crate::{
    ast_id_map::SyntaxFileId,
    body::{Body, BodySourceMap},
    db::HirDefDb,
    lower_ident_opt,
    source_map::Lowered,
};

pub mod config;
pub mod library;
pub mod udp;

pub(crate) type LowerFileCtx<'a> = LoweringCtx<FileStore<'a>>;

impl LowerFileCtx<'_> {
    fn lower_struct_type(&mut self, struct_ty: ast::StructUnionType) -> StructId {
        let container_id = self.current_owner();
        let struct_def = lower_struct_def(struct_ty, container_id, |ty| self.lower_data_ty(ty));

        alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.structs,
            &mut self.store.sources.struct_srcs,
            struct_def,
            struct_ty,
        )
    }

    fn lower_typedef(&mut self, typedef: ast::TypedefDeclaration) -> TypedefId {
        let name = lower_ident_opt(typedef.name());
        let typedef_id = alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.typedefs,
            &mut self.store.sources.typedef_srcs,
            Typedef { name, ty: None },
            typedef,
        );
        self.record_body_typedef(typedef_id);

        let data_ty = typedef.type_();
        let lowered_ty = lower_typedef_data_ty(
            self,
            data_ty,
            self.current_owner(),
            |ctx, struct_ty| ctx.lower_struct_type(struct_ty),
            |ctx, ty| ctx.lower_data_ty(ty),
        );

        self.store.data.typedefs[typedef_id].ty = Some(lowered_ty);

        typedef_id
    }

    fn lower_subroutine_decl(
        &mut self,
        func: ast::FunctionDeclaration,
    ) -> Option<LocalSubroutineId> {
        // Only the skeleton is lowered here; the body is lowered on first
        // access by subroutine_body_with_source_map.
        let subroutine = lower_subroutine(&func, |ty| self.lower_data_ty(ty))?;

        let local_subroutine_id = alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.subroutines,
            &mut self.store.sources.subroutine_srcs,
            subroutine,
            func,
        );

        Some(local_subroutine_id)
    }

    fn lower_config_decl(&mut self, config_decl: ast::ConfigDeclaration) -> ConfigDeclId {
        let name = lower_ident_opt(config_decl.name());

        alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.config_decls,
            &mut self.store.sources.config_decl_srcs,
            ConfigDecl { name },
            config_decl,
        )
    }

    fn lower_udp_decl(&mut self, udp_decl: ast::UdpDeclaration) -> UdpDeclId {
        let name = lower_ident_opt(udp_decl.name());

        alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.udp_decls,
            &mut self.store.sources.udp_decl_srcs,
            UdpDecl { name },
            udp_decl,
        )
    }

    fn lower_library_decl(&mut self, library_decl: ast::LibraryDeclaration) -> LibraryDeclId {
        let name = lower_ident_opt(library_decl.name());

        alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.library_decls,
            &mut self.store.sources.library_decl_srcs,
            LibraryDecl { name },
            library_decl,
        )
    }

    fn lower_library_include(
        &mut self,
        library_include: ast::LibraryIncludeStatement,
    ) -> LibraryIncludeId {
        alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.library_includes,
            &mut self.store.sources.library_include_srcs,
            LibraryInclude,
            library_include,
        )
    }

    fn lower_covergroup_decl(
        &mut self,
        covergroup_decl: ast::CovergroupDeclaration,
    ) -> CovergroupId {
        let mut covergroup = lower_covergroup_decl(covergroup_decl);

        for member in covergroup_decl.members().children() {
            match member {
                ast::Member::Coverpoint(coverpoint_ast) => {
                    let coverpoint = lower_coverpoint(coverpoint_ast);
                    let coverpoint_id = alloc_with_source(
                        &self.ast_ids,
                        &self.tree,
                        &mut self.store.data.coverpoints,
                        &mut self.store.sources.coverpoint_srcs,
                        coverpoint,
                        coverpoint_ast,
                    );
                    covergroup.coverpoints.push(coverpoint_id);
                }
                ast::Member::CoverCross(cross_ast) => {
                    let cross = lower_cross(cross_ast);
                    let cross_id = alloc_with_source(
                        &self.ast_ids,
                        &self.tree,
                        &mut self.store.data.crosses,
                        &mut self.store.sources.cross_srcs,
                        cross,
                        cross_ast,
                    );
                    covergroup.crosses.push(cross_id);
                }
                _ => {}
            }
        }

        alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.covergroups,
            &mut self.store.sources.covergroup_srcs,
            covergroup,
            covergroup_decl,
        )
    }

    pub(crate) fn lower_file(&mut self, root: ast::CompilationUnit) {
        for member in root.members().children() {
            use ast::Member::*;
            let idx = match member {
                ModuleDeclaration(decl) => {
                    let name = lower_ident_opt(decl.header().name());
                    let kind = ModuleKind::from_ast(decl);

                    alloc_with_source(
                        &self.ast_ids,
                        &self.tree,
                        &mut self.store.data.modules,
                        &mut self.store.sources.module_srcs,
                        ModuleInfo { name, kind },
                        decl,
                    )
                    .into()
                }
                ProceduralBlock(proc) => self.lower_proc(proc).into(),
                DataDeclaration(data_decl) => self.lower_data_decl(data_decl).into(),
                NetDeclaration(net_decl) => self.lower_net_decl(net_decl).into(),
                EmptyMember(_x) => continue,
                TypedefDeclaration(typedef_decl) => self.lower_typedef(typedef_decl).into(),
                FunctionDeclaration(fn_decl) => match self.lower_subroutine_decl(fn_decl) {
                    Some(id) => id.into(),
                    None => continue,
                },
                PackageImportDeclaration(import_decl) => {
                    for import in lower_package_imports(import_decl) {
                        self.store.data.package_imports.alloc(import);
                    }
                    continue;
                }
                UdpDeclaration(udp_decl) => self.lower_udp_decl(udp_decl).into(),
                ConfigDeclaration(config_decl) => self.lower_config_decl(config_decl).into(),
                CheckerDeclaration(checker_decl) => self.lower_checker_decl(checker_decl).into(),
                CovergroupDeclaration(covergroup_decl) => {
                    self.lower_covergroup_decl(covergroup_decl).into()
                }
                _ => continue,
            };
            self.store.data.items.push(idx);
        }
    }

    pub(crate) fn lower_library_map(&mut self, root: ast::LibraryMap) {
        for member in root.members().children() {
            use ast::Member::*;
            let idx = match member {
                LibraryDeclaration(library_decl) => self.lower_library_decl(library_decl).into(),
                LibraryIncludeStatement(library_include) => {
                    self.lower_library_include(library_include).into()
                }
                EmptyMember(_) => continue,
                _ => continue,
            };
            self.store.data.items.push(idx);
        }
    }
}

pub(crate) fn lower_file_owner(
    owner: crate::owner::OwnerId,
    syntax: &LoweringSyntax,
) -> Arc<Lowered<Body>> {
    let file_id = syntax.file_id;
    let tree = syntax.tree.clone();
    let mut body = Body::default();
    let mut source_map = BodySourceMap::default();
    let mut lower_ctx = LoweringCtx::new_with_syntax(
        owner,
        syntax,
        FileStore { data: &mut body, sources: &mut source_map },
    );
    match tree.root() {
        Some(root) if ast::CompilationUnit::can_cast(root.kind()) => {
            if let Some(root) = ast::CompilationUnit::cast(root) {
                lower_ctx.lower_file(root);
            }
        }
        Some(root) if ast::LibraryMap::can_cast(root.kind()) => {
            if let Some(root) = ast::LibraryMap::cast(root) {
                lower_ctx.lower_library_map(root);
            }
        }
        _ => {}
    }

    let diagnostics = lower_ctx.emit_diagnostics();
    drop(lower_ctx);
    body.shrink_to_fit();
    source_map.shrink_to_fit();
    Arc::new(Lowered::new_with_diagnostics(file_id, body, source_map, diagnostics))
}
#[salsa::tracked(lru = 128, returns(clone))]
fn hir_file_input(db: &dyn HirDefDb, file: SyntaxFileId) -> Arc<Lowered<Body>> {
    let file_id = file.hir_file(db);
    let owner = db.owner_table(file_id).file_owner().expect("file owner must exist");
    lower_file_owner(owner, &LoweringSyntax::for_owner(db, owner))
}

pub(crate) fn set_hir_file_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    hir_file_input::set_lru_capacity(db, capacity);
    hir_file_with_source_map::set_lru_capacity(db, capacity);
}

#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn hir_file_with_source_map(
    db: &dyn HirDefDb,
    file: SyntaxFileId,
) -> Arc<Lowered<Body>> {
    hir_file_input(db, file)
}
