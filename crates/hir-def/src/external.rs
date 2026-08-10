use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{
    TokenKind,
    ast::{self, AstNode, PortExpression},
};

use crate::{
    Ident, PackageImport, alloc_with_source_entry,
    declaration::ParamDeclKind,
    expr::{ExprId, data_ty::DataTy},
    lower::{BodyStore, LoweringCtx, LoweringStore},
    lower_ident_opt, lower_package_imports,
    module::{ModuleKind, port::PortHeader},
    subroutine::{Subroutine, lower_subroutine_prototype},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ExternInterfaceMethod {
    pub method: Subroutine,
    pub fork_join: bool,
}

pub type ExternInterfaceMethodId = Idx<ExternInterfaceMethod>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ExternParameter {
    Type { name: Ident, default: Option<DataTy> },
    Value { kind: ParamDeclKind, name: Ident, ty: DataTy, default: Option<ExprId> },
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ExternModulePort {
    pub header: PortHeader,
    pub name: Option<Ident>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ExternNonAnsiPort {
    pub label: Option<Ident>,
    pub references: SmallVec<[Ident; 2]>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ExternModulePortList {
    Ansi(SmallVec<[ExternModulePort; 4]>),
    NonAnsi(SmallVec<[ExternNonAnsiPort; 4]>),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ExternModuleDecl {
    pub kind: ModuleKind,
    pub name: Ident,
    pub imports: SmallVec<[PackageImport; 2]>,
    pub parameters: SmallVec<[ExternParameter; 4]>,
    pub ports: Option<ExternModulePortList>,
}

pub type ExternModuleDeclId = Idx<ExternModuleDecl>;

impl LoweringCtx<BodyStore<'_>> {
    pub(crate) fn lower_extern_module_decl(
        &mut self,
        declaration: ast::ExternModuleDecl,
    ) -> Option<ExternModuleDeclId> {
        let header = declaration.header();
        let kind = ModuleKind::from_header(header);
        if kind == ModuleKind::Package {
            self.report_invalid(
                declaration.syntax(),
                "extern module declaration cannot declare a package",
            );
            return None;
        }
        let Some(name) = lower_ident_opt(header.name()) else {
            self.report_invalid(
                declaration.syntax(),
                "extern module declaration is missing its name",
            );
            return None;
        };
        let imports = header
            .imports()
            .children()
            .flat_map(|import| lower_package_imports(import, self.source_id(import.syntax())))
            .collect();
        let parameters = match header.parameters() {
            Some(parameters) => self.lower_extern_parameters(parameters)?,
            None => SmallVec::new(),
        };
        let ports = match header.ports() {
            Some(ports) => Some(self.lower_extern_ports(ports)?),
            None => None,
        };
        let source = self.source_id(declaration.syntax());
        let (body, sources) = self.store.body();
        Some(alloc_with_source_entry(
            &mut body.extern_module_decls,
            &mut sources.extern_module_decl_srcs,
            ExternModuleDecl { kind, name, imports, parameters, ports },
            source,
        ))
    }

    fn lower_extern_parameters(
        &mut self,
        parameters: ast::ParameterPortList,
    ) -> Option<SmallVec<[ExternParameter; 4]>> {
        let mut result = SmallVec::new();
        for declaration in parameters.declarations().children() {
            match declaration {
                ast::ParameterDeclarationBase::TypeParameterDeclaration(declaration) => {
                    for assignment in declaration.declarators().children() {
                        let Some(name) = lower_ident_opt(assignment.name()) else {
                            self.report_invalid(
                                assignment.syntax(),
                                "extern module type parameter is missing its name",
                            );
                            return None;
                        };
                        let default = assignment
                            .assignment()
                            .map(|assignment| self.lower_data_ty(assignment.type_()));
                        result.push(ExternParameter::Type { name, default });
                    }
                }
                ast::ParameterDeclarationBase::ParameterDeclaration(declaration) => {
                    let kind = match declaration.keyword().map(|token| token.kind()) {
                        Some(TokenKind::PARAMETER_KEYWORD) => ParamDeclKind::Parameter,
                        Some(TokenKind::LOCAL_PARAM_KEYWORD) => ParamDeclKind::LocalParam,
                        _ => {
                            self.report_invalid(
                                declaration.syntax(),
                                "extern module value parameter has an invalid kind",
                            );
                            return None;
                        }
                    };
                    let ty = self.lower_data_ty(declaration.type_());
                    for declarator in declaration.declarators().children() {
                        let Some(name) = lower_ident_opt(declarator.name()) else {
                            self.report_invalid(
                                declarator.syntax(),
                                "extern module value parameter is missing its name",
                            );
                            return None;
                        };
                        let default = declarator
                            .initializer()
                            .map(|initializer| self.lower_expr(initializer.expr()));
                        result.push(ExternParameter::Value { kind, name, ty: ty.clone(), default });
                    }
                }
            }
        }
        Some(result)
    }

    fn lower_extern_ports(&mut self, ports: ast::PortList) -> Option<ExternModulePortList> {
        match ports {
            ast::PortList::AnsiPortList(ports) => {
                let mut result = SmallVec::new();
                let mut previous = None;
                for port in ports.ports().children() {
                    use ast::Member::*;
                    let (header, name) = match port {
                        ImplicitAnsiPort(port) => {
                            let header = self.lower_port_header(port.header(), previous);
                            let name = lower_ident_opt(port.declarator().name());
                            (header, name)
                        }
                        ExplicitAnsiPort(port) => {
                            let header = self.lower_explicit_ansi_header(
                                port.direction(),
                                previous,
                                port.syntax(),
                            );
                            let name = lower_ident_opt(port.name());
                            (header, name)
                        }
                        _ => {
                            self.report_invalid(
                                port.syntax(),
                                "extern module ANSI port list contains an invalid member",
                            );
                            return None;
                        }
                    };
                    previous = Some(header.clone());
                    result.push(ExternModulePort { header, name });
                }
                Some(ExternModulePortList::Ansi(result))
            }
            ast::PortList::NonAnsiPortList(ports) => {
                let mut result = SmallVec::new();
                for port in ports.ports().children() {
                    let (label, expression) = match port {
                        ast::NonAnsiPort::ImplicitNonAnsiPort(port) => (None, Some(port.expr())),
                        ast::NonAnsiPort::ExplicitNonAnsiPort(port) => {
                            (lower_ident_opt(port.name()), port.expr())
                        }
                        ast::NonAnsiPort::EmptyNonAnsiPort(port) => {
                            self.report_invalid(
                                port.syntax(),
                                "extern module non-ANSI port list contains an empty port",
                            );
                            return None;
                        }
                    };
                    let references = match expression {
                        Some(expression) => self.lower_extern_port_expression(expression)?,
                        None => SmallVec::new(),
                    };
                    if label.is_none() && references.is_empty() {
                        self.report_invalid(
                            port.syntax(),
                            "extern module non-ANSI port is missing its name",
                        );
                        return None;
                    }
                    result.push(ExternNonAnsiPort { label, references });
                }
                Some(ExternModulePortList::NonAnsi(result))
            }
            ast::PortList::WildcardPortList(ports) => {
                self.report_invalid(
                    ports.syntax(),
                    "extern module declaration cannot use a wildcard port list",
                );
                None
            }
        }
    }

    fn lower_extern_port_expression(
        &mut self,
        expression: PortExpression,
    ) -> Option<SmallVec<[Ident; 2]>> {
        match expression {
            PortExpression::PortReference(reference) => {
                Some(smallvec::smallvec![lower_ident_opt(reference.name())?])
            }
            PortExpression::PortConcatenation(concatenation) => concatenation
                .references()
                .children()
                .map(|reference| lower_ident_opt(reference.name()))
                .collect(),
        }
    }

    pub(crate) fn lower_extern_interface_method(
        &mut self,
        declaration: ast::ExternInterfaceMethod,
    ) -> Option<ExternInterfaceMethodId> {
        let prototype = declaration.prototype();
        let Some(keyword) = prototype.keyword() else {
            self.report_invalid(
                declaration.syntax(),
                "extern interface method is missing its subroutine kind",
            );
            return None;
        };
        let is_task = match keyword.kind() {
            TokenKind::TASK_KEYWORD => true,
            TokenKind::FUNCTION_KEYWORD => false,
            _ => {
                self.report_invalid(
                    declaration.syntax(),
                    "extern interface method has an invalid subroutine kind",
                );
                return None;
            }
        };
        let ast_ids = self.ast_ids.clone();
        let tree = self.tree.clone();
        let Some(method) = lower_subroutine_prototype(
            prototype,
            is_task,
            false,
            |ty| self.lower_data_ty(ty),
            &ast_ids,
            &tree,
        ) else {
            self.report_invalid(
                declaration.syntax(),
                "extern interface method has an invalid prototype",
            );
            return None;
        };
        let fork_join = match declaration.fork_join() {
            None => false,
            Some(token) if token.kind() == TokenKind::FORK_JOIN_KEYWORD => true,
            Some(_) => {
                self.report_invalid(
                    declaration.syntax(),
                    "extern interface method has an invalid forkjoin qualifier",
                );
                return None;
            }
        };
        let source = self.source_id(declaration.syntax());
        let (body, sources) = self.store.body();
        Some(alloc_with_source_entry(
            &mut body.extern_interface_methods,
            &mut sources.extern_interface_method_srcs,
            ExternInterfaceMethod { method, fork_join },
            source,
        ))
    }
}
