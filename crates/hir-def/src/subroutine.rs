use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{TokenKind, ast};

use super::{Ident, expr::data_ty::DataTy, lower_ident_opt};
use crate::{
    body::{Body, BodySourceMap},
    source_map::{AstKind, NamedAstId},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Subroutine {
    pub name: Option<Ident>,
    pub kind: SubroutineKind,
    pub ports: SmallVec<[SubroutinePort; 4]>,
    pub has_body: bool,
}

impl Default for Subroutine {
    fn default() -> Self {
        Subroutine {
            name: None,
            kind: SubroutineKind::Task,
            ports: SmallVec::new(),
            has_body: false,
        }
    }
}

/// Compatibility names for the canonical owner-local body storage.
pub type SubroutineBody = Body;
pub type SubroutineBodySourceMap = BodySourceMap;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SubroutineKind {
    Task,
    Function { return_ty: Option<DataTy> },
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SubroutinePort {
    pub direction: SubroutinePortDir,
    pub ty: Option<DataTy>,
    pub name: Option<Ident>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct SubroutinePortId(pub u32);

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum SubroutinePortDir {
    Input,
    Output,
    Inout,
    Ref,
    ConstRef,
    #[default]
    Unknown,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct FunctionDeclarationAst;

impl AstKind for FunctionDeclarationAst {
    type Node<'a> = ast::FunctionDeclaration<'a>;
}

pub type SubroutineSrc = NamedAstId<FunctionDeclarationAst>;

pub type LocalSubroutineId = Idx<Subroutine>;

pub fn lower_subroutine<F>(func: &ast::FunctionDeclaration, mut lower_ty: F) -> Option<Subroutine>
where
    F: FnMut(ast::DataType) -> DataTy,
{
    let prototype = func.prototype();
    let name = lower_name(prototype.name())?;

    let is_task = func.as_task_declaration().is_some();

    let mut ports = SmallVec::<[SubroutinePort; 4]>::new();
    if let Some(port_list) = prototype.port_list() {
        for port_base in port_list.ports().children() {
            if let Some(port) = port_base.as_function_port() {
                let mut dir = map_direction(port.direction().map(|tok| tok.kind()));
                if matches!(dir, SubroutinePortDir::Ref) && port.const_keyword().is_some() {
                    dir = SubroutinePortDir::ConstRef;
                }

                let ty = port.data_type().map(&mut lower_ty);
                let name = lower_ident_opt(port.declarator().name());
                ports.push(SubroutinePort { direction: dir, ty, name });
            } else if port_base.as_default_function_port().is_some() {
                ports.push(SubroutinePort {
                    direction: SubroutinePortDir::Input,
                    ty: None,
                    name: None,
                });
            }
        }
    }

    let kind = if is_task {
        SubroutineKind::Task
    } else {
        let ret_ty = lower_ty(prototype.return_type());
        SubroutineKind::Function { return_ty: Some(ret_ty) }
    };

    Some(Subroutine { name: Some(name), kind, ports, has_body: func.end().is_some() })
}

fn lower_name(name: ast::Name) -> Option<Ident> {
    if let Some(id) = name.as_identifier_name().and_then(|n| n.identifier()) {
        return lower_ident_opt(Some(id));
    }
    if let Some(select) = name.as_identifier_select_name() {
        return select.identifier().and_then(|tok| lower_ident_opt(Some(tok)));
    }
    if let Some(scoped) = name.as_scoped_name() {
        return lower_name(scoped.right());
    }
    None
}

fn map_direction(kind: Option<TokenKind>) -> SubroutinePortDir {
    match kind {
        Some(TokenKind::OUTPUT_KEYWORD) => SubroutinePortDir::Output,
        Some(TokenKind::IN_OUT_KEYWORD) => SubroutinePortDir::Inout,
        Some(TokenKind::REF_KEYWORD) => SubroutinePortDir::Ref,
        Some(TokenKind::INPUT_KEYWORD) | None => SubroutinePortDir::Input,
        Some(_) => SubroutinePortDir::Unknown,
    }
}
