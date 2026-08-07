use la_arena::IdxRange;
use port::{NonAnsiPortId, PortDeclId, Ports};
use syntax::ast::{self, AstNode, PortList};
use triomphe::Arc;

use super::{
    declaration::{Declaration, ParamDeclKind},
    expr::declarator::DeclId,
    lower::{BodyStore, LoweringCtx, LoweringSyntax},
    lower_ident_opt, lower_package_imports,
};
use crate::{
    body::{Body, BodyItem, BodySourceMap},
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

pub fn param_port_id_by_idx(body: &Body, idx: usize) -> Option<DeclId> {
    body.param_ports.clone()?.nth(idx)
}

pub fn overridable_param_id_by_idx(body: &Body, idx: usize) -> Option<DeclId> {
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

pub fn non_ansi_port_id_by_idx(body: &Body, idx: usize) -> Option<NonAnsiPortId> {
    let Ports::NonAnsi { ports, .. } = &body.ports else {
        return None;
    };
    ports.iter().nth(idx).map(|(port_id, _)| port_id)
}

pub fn ansi_port_decl_id_by_idx(body: &Body, idx: usize) -> Option<PortDeclId> {
    let Ports::Ansi(port_decls) = &body.ports else {
        return None;
    };
    port_decls.iter().nth(idx).map(|(port_decl_id, _)| port_decl_id)
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

pub(crate) type LowerModuleCtx<'a> = LoweringCtx<BodyStore<'a>>;

impl LowerModuleCtx<'_> {
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
            }

            let param_port_range = {
                let mut decls = self.store.data.decls.iter().map(|(id, _)| id);
                decls.next().map(|first| {
                    let last = decls.next_back().unwrap_or(first);
                    IdxRange::new_inclusive(first..=last)
                })
            };
            self.store.data.param_ports = param_port_range;
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

                DataDeclaration(data_decl) => {
                    let id = self.lower_data_decl(data_decl);
                    let decls = self.store.data.declarations[id].decls();
                    self.bind_nonansi_declarations(decls);
                    id.into()
                }
                NetDeclaration(net_decl) => {
                    let id = self.lower_net_decl(net_decl);
                    let decls = self.store.data.declarations[id].decls();
                    self.bind_nonansi_declarations(decls);
                    id.into()
                }
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
                    Some(owner) => BodyItem::SubroutineOwner(owner),
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

                TimeUnitsDeclaration(_) | ClockingItem(_) => continue,
                DefaultClockingReference(reference) => {
                    self.lower_default_clocking_reference(reference);
                    continue;
                }
                ClockingDeclaration(clocking) => {
                    let owner = self
                        .owner_for_node(clocking.syntax(), OwnerKind::ClockingBlock)
                        .expect("every lowered clocking block must have a canonical owner");
                    BodyItem::ClockingBlockOwner(owner)
                }

                // Assertions and properties
                PropertyDeclaration(_)
                | SequenceDeclaration(_)
                | ImmediateAssertionMember(_)
                | ConcurrentAssertionMember(_) => continue,

                // Coverage
                CovergroupDeclaration(covergroup) => {
                    let owner = self
                        .owner_for_node(covergroup.syntax(), OwnerKind::Covergroup)
                        .expect("every lowered covergroup must have a canonical owner");
                    BodyItem::CovergroupOwner(owner)
                }
                Coverpoint(_) | CoverCross(_) | CoverageBins(_) | BinsSelection(_)
                | CoverageOption(_) => continue,

                // Specify blocks
                DefaultSkewItem(_) => continue,
                SpecifyBlock(specify) => self.lower_specify_block(specify).into(),
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
                CheckerDeclaration(decl) => {
                    let owner = self
                        .owner_for_node(decl.syntax(), OwnerKind::Checker)
                        .expect("every lowered checker must have a canonical owner");
                    BodyItem::CheckerOwner(owner)
                }
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

                // Unsupported member kinds do not contribute to this owner's
                // structural HIR yet.
                _ => continue,
            };
            self.store.data.items.push(idx.clone());
        }
    }
}

pub(crate) fn lower_module_owner(
    db: &dyn HirDefDb,
    owner: OwnerId,
    syntax: &LoweringSyntax,
) -> Arc<Lowered<Body>> {
    debug_assert_eq!(owner.kind(db), OwnerKind::Module);
    let file_id = syntax.file_id;
    let tree = syntax.tree.clone();
    let mut body = Body::default();
    let mut source_map = BodySourceMap::default();

    let Some(ast_module) =
        syntax.ast_ids.node(owner.ast_id(db), &tree).and_then(ast::ModuleDeclaration::cast)
    else {
        return Arc::new(Lowered::new(file_id, body, source_map));
    };
    body.name = lower_ident_opt(ast_module.header().name());

    let mut lower_ctx = LoweringCtx::new_with_syntax(
        owner,
        syntax,
        BodyStore { data: &mut body, sources: &mut source_map },
    );
    lower_ctx.lower_module_decl(ast_module);
    let diagnostics = lower_ctx.emit_diagnostics();
    drop(lower_ctx);
    body.shrink_to_fit();
    source_map.shrink_to_fit();
    Arc::new(Lowered::new_with_diagnostics(file_id, body, source_map, diagnostics))
}
