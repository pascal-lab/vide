use la_arena::{Idx, IdxRange};
use port::{NonAnsiPortId, PortDeclId, Ports};
use syntax::{
    ast::{self, AstNode, PortList},
    has_name::HasName,
};
use triomphe::Arc;

use super::{
    Ident,
    aggregate::{StructId, lower_struct_def},
    alloc_with_source,
    covergroup::{CovergroupId, lower_covergroup_decl, lower_coverpoint, lower_cross},
    declaration::{Declaration, ParamDeclKind},
    expr::declarator::{DeclId, Declarator},
    lower::{LoweringCtx, ModuleStore},
    lower_ident_opt, lower_package_imports,
    subroutine::{LocalSubroutineId, lower_subroutine},
    typedef::{Typedef, TypedefId, lower_typedef_data_ty},
};
use crate::{
    body::{Body, BodyItem, BodySourceMap},
    container::InFile,
    db::HirDefDb,
    owner::{OwnerId, OwnerKind},
    source_map::Lowered,
};

pub mod clocking;
pub mod continuous_assign;
pub mod defparam;
pub mod generate;
pub mod instantiation;
pub mod modport;
pub mod port;
pub mod specify;

pub type Module = Body;
pub type ModuleSourceMap = BodySourceMap;
pub type ModuleItem = BodyItem;

pub type ModuleSrc = crate::ast_id_map::SourceAstId;

impl Module {
    pub fn param_port_id_by_idx(&self, idx: usize) -> Option<DeclId> {
        self.param_ports.clone()?.nth(idx)
    }

    pub fn overridable_param_id_by_idx(&self, body: &Body, idx: usize) -> Option<DeclId> {
        body.declarations
            .values()
            .filter_map(|declaration| match declaration {
                Declaration::ParamDecl(param_decl) if param_decl.kind.is_overridable() => {
                    Some(param_decl.decls.clone())
                }
                _ => None,
            })
            .flatten()
            .nth(idx)
    }

    pub fn non_ansi_port_id_by_idx(&self, idx: usize) -> Option<NonAnsiPortId> {
        let Ports::NonAnsi { ports, .. } = &self.ports else {
            return None;
        };
        ports.iter().nth(idx).map(|(port_id, _)| port_id)
    }

    pub fn ansi_port_decl_id_by_idx(&self, idx: usize) -> Option<PortDeclId> {
        let Ports::Ansi(port_decls) = &self.ports else {
            return None;
        };
        port_decls.iter().nth(idx).map(|(port_decl_id, _)| port_decl_id)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Default)]
pub enum ModuleKind {
    #[default]
    Module,
    Interface,
    Program,
    Package,
}

impl ModuleKind {
    pub fn from_ast(decl: ast::ModuleDeclaration) -> Self {
        if decl.as_package_declaration().is_some() {
            ModuleKind::Package
        } else if decl.as_interface_declaration().is_some() {
            ModuleKind::Interface
        } else if decl.as_program_declaration().is_some() {
            ModuleKind::Program
        } else {
            ModuleKind::Module
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ModuleInfo {
    pub name: Option<Ident>,
    pub kind: ModuleKind,
}

pub type LocalModuleId = Idx<ModuleInfo>;
pub type ModuleId = InFile<LocalModuleId>;
pub type PackageId = ModuleId;

pub(crate) type LowerModuleCtx<'a> = LoweringCtx<ModuleStore<'a>>;

impl LowerModuleCtx<'_> {
    fn lower_struct_type(&mut self, struct_ty: ast::StructUnionType) -> StructId {
        let container_id = self.current_arena_owner();
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
            self.current_arena_owner(),
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

        let subroutine_id = alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.subroutines,
            &mut self.store.sources.subroutine_srcs,
            subroutine,
            func,
        );

        Some(subroutine_id)
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

    pub(crate) fn lower_module_decl(&mut self, decl: ast::ModuleDeclaration) {
        let header = decl.header();
        let has_param_ports = header.parameters().is_some();
        if let Some(param_ports) = header.parameters() {
            let mut inherited_kind = ParamDeclKind::Parameter;
            for decls in param_ports.declarations().children() {
                let decl_id = self.lower_param_decl_base_with_context(
                    decls,
                    Some(inherited_kind),
                    false,
                    true,
                );
                if let Declaration::ParamDecl(param_decl) = &self.store.data.declarations[decl_id] {
                    inherited_kind = param_decl.kind;
                }
                self.region_tree.handle_node(decls.syntax());
            }

            let param_port_range = {
                let mut decls = self.store.data.decls.iter().map(|(id, _)| id);
                decls.next().map(|first| {
                    let last = decls.next_back().unwrap_or(first);
                    IdxRange::new_inclusive(first..=last)
                })
            };
            self.store.data.param_ports = param_port_range;

            self.region_tree.stage(param_ports.close_paren(), param_ports.syntax());
        }

        match header.ports() {
            Some(PortList::AnsiPortList(port_list)) => self.lower_ansi_ports(port_list),
            Some(PortList::NonAnsiPortList(port_list)) => self.lower_nonansi_port(port_list),
            Some(PortList::WildcardPortList(port_list)) => self.lower_wildcard_ports(port_list),
            None => {}
        };

        for member in decl.members().children() {
            use ast::Member::*;
            let idx: BodyItem = match member {
                // Assignments
                ContinuousAssign(assign) => self.lower_continuous_assign(assign).into(),

                // Declarations
                DataDeclaration(data_decl) => self.lower_data_decl(data_decl).into(),
                NetDeclaration(net_decl) => self.lower_net_decl(net_decl).into(),
                LocalVariableDeclaration(_) => continue,
                ParameterDeclarationStatement(param_decl) => self
                    .lower_param_decl_base_with_context(
                        param_decl.parameter(),
                        None,
                        has_param_ports,
                        false,
                    )
                    .into(),
                TypedefDeclaration(typedef_decl) => self.lower_typedef(typedef_decl).into(),
                GenvarDeclaration(genvar_decl) => self.lower_genvar_decl(genvar_decl).into(),
                NetTypeDeclaration(_)
                | ForwardTypedefDeclaration(_)
                | UserDefinedNetDeclaration(_) => {
                    continue;
                }

                // Instantiations
                HierarchyInstantiation(instantiation) => {
                    self.lower_instantiation(instantiation).into()
                }
                PrimitiveInstantiation(instantiation) => {
                    self.lower_primitive_instantiation(instantiation).into()
                }
                CheckerInstantiation(instantiation) => {
                    self.lower_checker_instantiation(instantiation).into()
                }

                // Subroutines
                FunctionDeclaration(fn_decl) => match self.lower_subroutine_decl(fn_decl) {
                    Some(sub_id) => sub_id.into(),
                    None => continue,
                },

                // Procedural blocks
                ProceduralBlock(proc) => self.lower_proc(proc).into(),

                // Ports
                PortDeclaration(port) => self.lower_port_decl(port).into(),
                ExplicitAnsiPort(_) | ImplicitAnsiPort(_) => continue,

                // Imports
                PackageImportDeclaration(import_decl) => {
                    for import in lower_package_imports(import_decl) {
                        self.store.data.package_imports.alloc(import);
                    }
                    continue;
                }

                // Aggregates
                ClassDeclaration(_) => continue,

                // Nested modules/interfaces/programs
                ModuleDeclaration(_) => continue,

                // Generate constructs
                GenerateRegion(region) => self.lower_generate_region(region).into(),
                gen_item @ GenerateBlock(_)
                | gen_item @ IfGenerate(_)
                | gen_item @ CaseGenerate(_)
                | gen_item @ LoopGenerate(_) => self.lower_direct_generate_region(gen_item).into(),

                // Timing and clocking
                TimeUnitsDeclaration(_) | ClockingItem(_) => continue,
                DefaultClockingReference(reference) => {
                    self.lower_default_clocking_reference(reference);
                    self.region_tree.handle_node(member.syntax());
                    continue;
                }
                ClockingDeclaration(clocking) => self.lower_clocking_declaration(clocking).into(),

                // Assertions and properties
                PropertyDeclaration(_)
                | SequenceDeclaration(_)
                | ImmediateAssertionMember(_)
                | ConcurrentAssertionMember(_) => continue,

                // Coverage
                CovergroupDeclaration(covergroup) => self.lower_covergroup_decl(covergroup).into(),
                Coverpoint(_) | CoverCross(_) | CoverageBins(_) | BinsSelection(_)
                | CoverageOption(_) => continue,

                // Specify blocks
                SpecifyBlock(block) => self.lower_specify_block(block).into(),
                PathDeclaration(path) => self.lower_specify_path_item(path).into(),
                ConditionalPathDeclaration(path) => {
                    self.lower_conditional_specify_path_item(path).into()
                }
                IfNonePathDeclaration(path) => self.lower_ifnone_specify_path_item(path).into(),
                SystemTimingCheck(timing) => self.lower_system_timing_check_item(timing).into(),
                PulseStyleDeclaration(pulse) => self.lower_pulse_style_item(pulse).into(),
                DefaultSkewItem(_) => continue,
                SpecparamDeclaration(specparam_decl) => {
                    self.lower_specparam_decl(specparam_decl).into()
                }

                // DPI and external
                DPIImport(_)
                | DPIExport(_)
                | ExternInterfaceMethod(_)
                | ExternModuleDecl(_)
                | ExternUdpDecl(_) => continue,

                // UDP
                UdpDeclaration(_) => continue,

                // Defparam
                DefParam(defparam) => self.lower_defparam(defparam).into(),

                // Net alias
                NetAlias(_) => continue,

                // Modport
                ModportDeclaration(modport) => {
                    for modport_id in self.lower_modport_declaration(modport) {
                        let item = BodyItem::from(modport_id);
                        self.store.data.items.push(item.clone());
                    }
                    self.region_tree.handle_node(member.syntax());
                    continue;
                }
                ModportClockingPort(_)
                | ModportSimplePortList(_)
                | ModportSubroutinePortList(_) => continue,

                // Class members (shouldn't appear in module but handle anyway)
                ClassPropertyDeclaration(_)
                | ClassMethodDeclaration(_)
                | ClassMethodPrototype(_) => continue,

                // Checker
                CheckerDeclaration(checker_decl) => self.lower_checker_decl(checker_decl).into(),
                CheckerDataDeclaration(_) => continue,

                // Constraints
                ConstraintDeclaration(_) | ConstraintPrototype(_) => continue,

                // Config
                ConfigDeclaration(_) => continue,

                // Bind
                BindDirective(_) => continue,

                // Package exports
                PackageExportDeclaration(_) | PackageExportAllDeclaration(_) => continue,

                // Library
                LibraryDeclaration(_) | LibraryIncludeStatement(_) => continue,

                // Let declaration
                LetDeclaration(_) => continue,

                // Default disable
                DefaultDisableDeclaration(_) => continue,

                // Elaboration system task
                ElabSystemTask(_) => continue,

                // Anonymous program
                AnonymousProgram(_) => continue,

                // Empty member - skip
                EmptyMember(_) => continue,
            };
            self.store.data.items.push(idx.clone());
            self.region_tree.handle_node(member.syntax());
        }
        self.region_tree.stage(decl.endmodule(), decl.syntax());
        self.store.sources.region_tree = self.region_tree.finish();
    }
}

#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn module_with_source_map(db: &dyn HirDefDb, owner: OwnerId) -> Arc<Lowered<Module>> {
    debug_assert_eq!(owner.kind(db), OwnerKind::Module);
    let file_id = owner.file(db);
    let tree = db.parse(file_id);
    let mut body = Body::default();
    let mut source_map = BodySourceMap::default();

    let Some(ast_module) =
        db.ast_id_map(file_id).node(owner.ast_id(db), &tree).and_then(ast::ModuleDeclaration::cast)
    else {
        return Arc::new(Lowered::new(file_id, body, source_map));
    };
    body.name = lower_ident_opt(ast_module.header().name());

    let mut lower_ctx =
        LoweringCtx::new(db, owner, ModuleStore { data: &mut body, sources: &mut source_map });
    lower_ctx.lower_module_decl(ast_module);
    let diagnostics = lower_ctx.emit_diagnostics();
    drop(lower_ctx);
    source_map.diagnostics = diagnostics;
    body.shrink_to_fit();
    source_map.shrink_to_fit();
    Arc::new(Lowered::new(file_id, body, source_map))
}

pub(crate) fn set_module_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    module_with_source_map::set_lru_capacity(db, capacity);
}
