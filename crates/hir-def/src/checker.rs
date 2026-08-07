use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{
    SyntaxTokenWithParent, TokenKind,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};
use utils::text_edit::TextRange;

use super::{
    alloc_with_source,
    declaration::DeclarationId,
    lower::{CheckerStore, LoweringCtx},
    module::port::PortDirection,
};
use crate::{Ident, ast_id_map::SourceAstId, lower_ident_opt};

// slang AST survey:
// - `CheckerDeclaration` owns assertion-item ports through
//   `port_list().ports()`.
// - Checker body variables arrive either as `Member::CheckerDataDeclaration`
//   wrapping ordinary `DataDeclaration` syntax, or directly as module-like
//   data/net declaration members depending on the concrete grammar form.

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CheckerDef {
    pub name: Option<Ident>,
    pub ports: SmallVec<[CheckerPort; 4]>,
    pub declarations: SmallVec<[DeclarationId; 4]>,
}

pub type CheckerId = Idx<CheckerDef>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CheckerPort {
    pub name: Ident,
    pub dir: PortDirection,
    pub name_range: Option<TextRange>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct CheckerPortId(pub u32);

pub type CheckerSrc = SourceAstId;

pub fn lower_checker_decl(checker: ast::CheckerDeclaration<'_>) -> CheckerDef {
    CheckerDef {
        name: lower_ident_opt(checker.name()),
        ports: lower_checker_ports(checker),
        declarations: SmallVec::new(),
    }
}

impl<Store: CheckerStore> LoweringCtx<Store> {
    pub(crate) fn lower_checker_decl(
        &mut self,
        checker_decl: ast::CheckerDeclaration<'_>,
    ) -> CheckerId {
        let mut checker = lower_checker_decl(checker_decl);
        lower_checker_declarations(&mut checker, checker_decl, |member| match member {
            CheckerDeclarationMember::Data(data_decl) => self.lower_data_decl(data_decl),
            CheckerDeclarationMember::Net(net_decl) => self.lower_net_decl(net_decl),
        });

        let _file_id = self.file_id;
        let (checkers, sources) = self.store.checkers();
        alloc_with_source(&self.ast_ids, &self.tree, checkers, sources, checker, checker_decl)
    }
}

fn lower_checker_ports(checker: ast::CheckerDeclaration<'_>) -> SmallVec<[CheckerPort; 4]> {
    let mut ports = SmallVec::new();
    let syntax = checker.syntax();
    let Some(port_list) = checker.port_list() else {
        return ports;
    };

    for port in port_list.ports().children() {
        let name_range = port
            .name()
            .and_then(|name| SyntaxTokenWithParent { parent: syntax, tok: name }.text_range());
        let Some(name) = lower_ident_opt(port.name()) else {
            continue;
        };
        ports.push(CheckerPort {
            name,
            dir: lower_checker_port_direction(port.direction()),
            name_range,
        });
    }

    ports
}

fn lower_checker_declarations(
    checker: &mut CheckerDef,
    checker_decl: ast::CheckerDeclaration<'_>,
    mut lower_member: impl FnMut(CheckerDeclarationMember<'_>) -> DeclarationId,
) {
    for member in checker_decl.members().children() {
        match member {
            ast::Member::CheckerDataDeclaration(data_decl) => {
                checker
                    .declarations
                    .push(lower_member(CheckerDeclarationMember::Data(data_decl.data())));
            }
            ast::Member::DataDeclaration(data_decl) => {
                checker.declarations.push(lower_member(CheckerDeclarationMember::Data(data_decl)));
            }
            ast::Member::NetDeclaration(net_decl) => {
                checker.declarations.push(lower_member(CheckerDeclarationMember::Net(net_decl)));
            }
            _ => {}
        }
    }
}

enum CheckerDeclarationMember<'a> {
    Data(ast::DataDeclaration<'a>),
    Net(ast::NetDeclaration<'a>),
}

fn lower_checker_port_direction(direction: Option<syntax::SyntaxToken<'_>>) -> PortDirection {
    direction
        .and_then(|direction| match direction.kind() {
            TokenKind::INPUT_KEYWORD => Some(PortDirection::Input),
            TokenKind::OUTPUT_KEYWORD => Some(PortDirection::Output),
            TokenKind::IN_OUT_KEYWORD => Some(PortDirection::Inout),
            TokenKind::REF_KEYWORD => Some(PortDirection::Ref),
            _ => None,
        })
        .unwrap_or(PortDirection::Input)
}
