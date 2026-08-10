use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{TokenKind, ast, ast::AstNode};

use crate::{
    Ident, alloc_with_source_entry,
    expr::Selector,
    lower::{LoweringCtx, ModuleItemStore},
    lower_ident_opt,
    module::instantiation::InstantiationId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindPathKind {
    Simple,
    Package,
    Hierarchical,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BindNameSegment {
    pub name: Ident,
    pub selectors: SmallVec<[Selector; 2]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BindName {
    pub segments: SmallVec<[BindNameSegment; 2]>,
    pub kind: BindPathKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindInstantiationKind {
    Hierarchy,
    Checker,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindDirective {
    pub target: BindName,
    pub target_instances: SmallVec<[BindName; 2]>,
    pub instantiation: InstantiationId,
    pub instantiation_kind: BindInstantiationKind,
}

pub type BindDirectiveId = Idx<BindDirective>;

impl<Store: ModuleItemStore> LoweringCtx<Store> {
    pub(crate) fn lower_bind_directive(
        &mut self,
        directive: ast::BindDirective,
    ) -> Option<BindDirectiveId> {
        let target = self.lower_bind_name(directive.target())?;
        let target_instances = if let Some(target_list) = directive.target_instances() {
            let mut targets = SmallVec::new();
            for target in target_list.targets().children() {
                targets.push(self.lower_bind_name(target)?);
            }
            targets
        } else {
            SmallVec::new()
        };

        let (instantiation, instantiation_kind) = match directive.instantiation() {
            ast::Member::HierarchyInstantiation(instantiation) => {
                (self.lower_instantiation(instantiation), BindInstantiationKind::Hierarchy)
            }
            ast::Member::CheckerInstantiation(instantiation) => {
                (self.lower_checker_instantiation(instantiation), BindInstantiationKind::Checker)
            }
            unsupported => {
                self.report_unsupported(
                    unsupported.syntax(),
                    "bind directive instantiation is not lowered",
                );
                return None;
            }
        };

        let source = self.source_id(directive.syntax());
        let (body, sources) = self.store.body();
        Some(alloc_with_source_entry(
            &mut body.bind_directives,
            &mut sources.bind_directive_srcs,
            BindDirective { target, target_instances, instantiation, instantiation_kind },
            source,
        ))
    }

    fn lower_bind_name(&mut self, name: ast::Name) -> Option<BindName> {
        let mut segments = SmallVec::new();
        let mut kind = None;
        self.lower_bind_name_parts(name, &mut segments, &mut kind)?;
        Some(BindName { segments, kind: kind.unwrap_or(BindPathKind::Simple) })
    }

    fn lower_bind_name_parts(
        &mut self,
        name: ast::Name,
        segments: &mut SmallVec<[BindNameSegment; 2]>,
        kind: &mut Option<BindPathKind>,
    ) -> Option<()> {
        match name {
            ast::Name::IdentifierName(name) => self.push_bind_name_segment(
                name.syntax(),
                name.identifier(),
                SmallVec::new(),
                segments,
            ),
            ast::Name::IdentifierSelectName(name) => {
                let mut selectors = SmallVec::new();
                for element in name.selectors().children() {
                    let Some(selector) = element.selector() else {
                        self.report_invalid(name.syntax(), "bind name has an invalid selector");
                        return None;
                    };
                    selectors.push(self.lower_selector(selector));
                }
                self.push_bind_name_segment(name.syntax(), name.identifier(), selectors, segments)
            }
            ast::Name::SystemName(name) => self.push_bind_name_segment(
                name.syntax(),
                name.system_identifier(),
                SmallVec::new(),
                segments,
            ),
            ast::Name::KeywordName(name) => self.push_bind_name_segment(
                name.syntax(),
                name.keyword(),
                SmallVec::new(),
                segments,
            ),
            ast::Name::ScopedName(name) => {
                let separator = match name.separator().map(|token| token.kind()) {
                    Some(TokenKind::DOUBLE_COLON) => BindPathKind::Package,
                    Some(TokenKind::DOT) => BindPathKind::Hierarchical,
                    Some(_) | None => {
                        self.report_invalid(name.syntax(), "bind name has an invalid separator");
                        return None;
                    }
                };
                if let Some(previous) = *kind {
                    if previous != separator {
                        self.report_unsupported(name.syntax(), "bind name mixes path separators");
                        return None;
                    }
                } else {
                    *kind = Some(separator);
                }
                self.lower_bind_name_parts(name.left(), segments, kind)?;
                self.lower_bind_name_parts(name.right(), segments, kind)
            }
            unsupported => {
                self.report_unsupported(unsupported.syntax(), "bind name is not lowered");
                None
            }
        }
    }

    fn push_bind_name_segment(
        &mut self,
        syntax: syntax::SyntaxNode<'_>,
        token: Option<syntax::SyntaxToken<'_>>,
        selectors: SmallVec<[Selector; 2]>,
        segments: &mut SmallVec<[BindNameSegment; 2]>,
    ) -> Option<()> {
        let Some(name) = lower_ident_opt(token) else {
            self.report_invalid(syntax, "bind name is missing an identifier");
            return None;
        };
        segments.push(BindNameSegment { name, selectors });
        Some(())
    }
}
