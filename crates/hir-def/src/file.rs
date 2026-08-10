use config::{ConfigDecl, ConfigDeclId, ConfigRule};
use library::{LibraryDecl, LibraryDeclId, LibraryInclude, LibraryIncludeId};
pub use preproc_expand::file::HirFileId;
use syntax::ast::{self, AstNode};
use triomphe::Arc;
use udp::{UdpDecl, UdpDeclId, UdpEntry, UdpInitialValue};

use super::{
    aggregate::{StructId, StructMember, lower_struct_def},
    alloc_with_source,
    lower::{BodyStore, LoweringCtx, LoweringSyntax},
    lower_package_imports,
};
use crate::{
    body::{Body, BodyItem, BodySourceMap},
    container::OwnerRef,
    db::HirDefDb,
    lower_ident_opt,
    owner::{OwnerId, OwnerKind},
    source_map::Lowered,
    typedef::{Typedef, TypedefId, lower_typedef_data_ty},
};

pub mod config;
pub mod library;
pub mod udp;

pub(crate) type LowerFileCtx<'a> = LoweringCtx<BodyStore<'a>>;

impl LowerFileCtx<'_> {
    fn lower_struct_type(&mut self, struct_ty: ast::StructUnionType) -> StructId {
        let container_id = self.current_owner();
        let struct_def = lower_struct_def(struct_ty, container_id, |member| {
            let member_ty = self.lower_data_ty(member.type_());
            member
                .declarators()
                .children()
                .map(|declarator| StructMember {
                    name: lower_ident_opt(declarator.name()),
                    ty: Some(OwnerRef::new(container_id, member_ty.clone())),
                    dimensions: declarator
                        .dimensions()
                        .children()
                        .map(|dim| self.lower_dimension(dim))
                        .collect(),
                    initializer: declarator.initializer().map(|init| self.lower_expr(init.expr())),
                    random: member.random_qualifier().is_some(),
                })
                .collect()
        });

        alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.structs,
            &mut self.store.sources.struct_srcs,
            struct_def,
            struct_ty,
        )
    }

    pub(crate) fn lower_typedef(&mut self, typedef: ast::TypedefDeclaration) -> TypedefId {
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

    pub(crate) fn lower_subroutine_decl(
        &mut self,
        func: ast::FunctionDeclaration,
    ) -> Option<OwnerId> {
        // The signature and body are lowered by the owner-local body query.
        self.owner_for_node(func.syntax(), OwnerKind::Subroutine)
    }

    fn lower_config_decl(&mut self, config_decl: ast::ConfigDeclaration) -> ConfigDeclId {
        let rules = config_decl
            .rules()
            .children()
            .map(|rule| ConfigRule { kind: rule.syntax().kind() })
            .collect();
        let top_cells = config_decl
            .top_cells()
            .children()
            .filter_map(|cell| cell.cell().and_then(|name| crate::lower_ident_opt(Some(name))))
            .collect();
        alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.config_decls,
            &mut self.store.sources.config_decl_srcs,
            ConfigDecl { name: lower_ident_opt(config_decl.name()), top_cells, rules },
            config_decl,
        )
    }

    fn lower_udp_decl(&mut self, udp_decl: ast::UdpDeclaration) -> UdpDeclId {
        let ports = match udp_decl.port_list() {
            ast::UdpPortList::AnsiUdpPortList(list) => list
                .ports()
                .children()
                .flat_map(|port| match port {
                    ast::UdpPortDecl::UdpOutputPortDecl(port) => {
                        vec![port.name()]
                    }
                    ast::UdpPortDecl::UdpInputPortDecl(port) => {
                        port.names().children().map(|name| name.identifier()).collect()
                    }
                })
                .filter_map(crate::lower_ident_opt)
                .collect(),
            ast::UdpPortList::NonAnsiUdpPortList(list) => list
                .ports()
                .children()
                .filter_map(|name| crate::lower_ident_opt(name.identifier()))
                .collect(),
            ast::UdpPortList::WildcardUdpPortList(_) => smallvec::SmallVec::new(),
        };
        let body = udp_decl.body();
        let initial = body
            .initial_stmt()
            .map(|stmt| UdpInitialValue { name: crate::lower_ident_opt(stmt.name()) });
        let entries = body
            .entries()
            .children()
            .map(|entry| UdpEntry {
                input_kinds: entry.inputs().children().map(|field| field.syntax().kind()).collect(),
                current: entry.current().map(|field| field.syntax().kind()),
                next: entry.next().map(|field| field.syntax().kind()),
            })
            .collect();
        alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.udp_decls,
            &mut self.store.sources.udp_decl_srcs,
            UdpDecl { name: lower_ident_opt(udp_decl.name()), ports, initial, entries },
            udp_decl,
        )
    }

    fn lower_library_decl(&mut self, library_decl: ast::LibraryDeclaration) -> LibraryDeclId {
        let file_paths = library_decl
            .file_paths()
            .children()
            .filter_map(|path| crate::lower_ident_opt(path.path()))
            .collect();
        let include_dirs = library_decl
            .inc_dir_clause()
            .into_iter()
            .flat_map(|clause| clause.file_paths().children().collect::<Vec<_>>())
            .filter_map(|path| crate::lower_ident_opt(path.path()))
            .collect();
        alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.library_decls,
            &mut self.store.sources.library_decl_srcs,
            LibraryDecl { name: lower_ident_opt(library_decl.name()), file_paths, include_dirs },
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
            LibraryInclude {
                file_path: crate::lower_ident_opt(library_include.file_path().path()),
            },
            library_include,
        )
    }

    pub(crate) fn lower_file(&mut self, root: ast::CompilationUnit) {
        for member in root.members().children() {
            use ast::Member::*;
            let idx: BodyItem = match member {
                ModuleDeclaration(decl) => {
                    let owner = self
                        .owner_for_node(decl.syntax(), crate::owner::OwnerKind::Module)
                        .expect("every lowered module must have a canonical owner");
                    BodyItem::ModuleOwner(owner)
                }
                ProceduralBlock(proc) => self.lower_proc(proc).into(),
                DataDeclaration(data_decl) => self.lower_data_decl(data_decl).into(),
                NetDeclaration(net_decl) => self.lower_net_decl(net_decl).into(),
                EmptyMember(_x) => continue,
                TypedefDeclaration(typedef_decl) => self.lower_typedef(typedef_decl).into(),
                ParameterDeclarationStatement(param_decl) => {
                    self.lower_param_decl_base(param_decl.parameter()).into()
                }
                ClassDeclaration(class) => self.lower_class_decl(class).into(),
                FunctionDeclaration(fn_decl) => match self.lower_subroutine_decl(fn_decl) {
                    Some(owner) => BodyItem::SubroutineOwner(owner),
                    None => continue,
                },
                PackageImportDeclaration(import_decl) => {
                    for import in
                        lower_package_imports(import_decl, self.source_id(import_decl.syntax()))
                    {
                        self.store.data.package_imports.alloc(import);
                    }
                    continue;
                }
                UdpDeclaration(udp_decl) => self.lower_udp_decl(udp_decl).into(),
                ConfigDeclaration(config_decl) => self.lower_config_decl(config_decl).into(),
                CheckerDeclaration(decl) => {
                    let owner = self
                        .owner_for_node(decl.syntax(), OwnerKind::Checker)
                        .expect("every lowered checker must have a canonical owner");
                    BodyItem::CheckerOwner(owner)
                }
                CovergroupDeclaration(decl) => {
                    let owner = self
                        .owner_for_node(decl.syntax(), OwnerKind::Covergroup)
                        .expect("every lowered covergroup must have a canonical owner");
                    BodyItem::CovergroupOwner(owner)
                }
                unsupported => {
                    self.report_unsupported(
                        unsupported.syntax(),
                        "unsupported compilation-unit member",
                    );
                    continue;
                }
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
                unsupported => {
                    self.report_unsupported(unsupported.syntax(), "unsupported library-map member");
                    continue;
                }
            };
            self.store.data.items.push(idx);
        }
    }
}

pub(crate) fn lower_file_owner(
    db: &dyn HirDefDb,
    owner: crate::owner::OwnerId,
    syntax: &LoweringSyntax,
) -> Arc<Lowered<Body>> {
    let file_id = syntax.file_id;
    let tree = syntax.tree.clone();
    let mut body = Body::default();
    let mut source_map = BodySourceMap::default();
    let mut lower_ctx = LoweringCtx::new_with_syntax(
        db,
        owner,
        syntax,
        BodyStore { data: &mut body, sources: &mut source_map },
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
