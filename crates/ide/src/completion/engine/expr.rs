use std::collections::BTreeMap;

use hir_def::{
    container::{OwnerRef, ScopeParent},
    def_id::DefId,
    lower_ident_opt,
    owner::{OwnerId, OwnerKind},
    symbol::{DefKind, Resolution},
};
use hir_semantics::semantics::Semantics;
use hir_ty::{Type, TypeSystem};
use preproc_expand::file::HirFileId;
use syntax::{
    SyntaxKind, SyntaxNode, SyntaxNodeExt,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};
use utils::text_edit::TextSize;

use super::{candidate::CompletionCandidate, system, typed_filter::is_compatible_typed_value};
use crate::analysis::AnalysisContext;
use crate::{FilePosition, completion::context::CompletionContext, db::root_db::RootDb};

#[derive(Clone, Debug)]
enum NameKind {
    Value { ty: Type },
    SubroutineCall { return_ty: Type },
}

pub(super) fn complete_expression(
    db: &AnalysisContext<'_>,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
    complete_expression_impl(db, position, prefix, ctx)
}

pub(super) fn complete_argument_exprs(
    db: &AnalysisContext<'_>,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
    complete_expression_impl(db, position, prefix, ctx)
}

fn complete_expression_impl(
    db: &AnalysisContext<'_>,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
    let sema = db.semantics();
    let file_id = position.file_id.into();
    let parsed_file = sema.parse_file(position.file_id);
    let Some(root) = parsed_file.root() else {
        return Vec::new();
    };

    let mut names: BTreeMap<String, NameKind> = BTreeMap::new();
    let mut current_module_id = None;

    if let Some(container_id) = container_id_at_offset(&sema, file_id, root, position.offset) {
        current_module_id = module_id_for_container(db, container_id);
        for container_id in ScopeParent::start_from(db.db, container_id) {
            collect_container_names(db, container_id, &mut names);
        }
    }

    let expected_ty = current_module_id.and_then(|module_id| {
        expected_type_at_offset(db, &sema, file_id, root, position.offset, module_id)
    });

    let mut candidates: Vec<_> = names
        .into_iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .filter(|(_, kind)| {
            expression_candidate_matches_expected_type(db, expected_ty.as_ref(), kind)
        })
        .map(|(name, kind)| match kind {
            NameKind::Value { .. } => CompletionCandidate::text(name, ctx.replacement),
            NameKind::SubroutineCall { .. } => CompletionCandidate::semantic_snippet(
                name.clone(),
                ctx.replacement,
                format!("{name}()"),
                format!("{name}(${{1:args}})"),
            ),
        })
        .collect();
    candidates.extend(system::complete_system_functions(prefix, ctx));
    candidates
}

fn container_id_at_offset(
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    root: SyntaxNode<'_>,
    offset: TextSize,
) -> Option<OwnerId> {
    let elem = root.covering_element(utils::line_index::TextRange::empty(offset));
    let node = elem.as_node().or_else(|| elem.parent())?;
    sema.container_for_node(file_id, node)
}

fn collect_container_names(db: &AnalysisContext<'_>, owner: OwnerId, names: &mut BTreeMap<String, NameKind>) {
    let scope = db.scope(owner);
    for (ident, defs) in scope.iter_listing() {
        collect_def_names(db, ident, defs, names);
    }
}

fn collect_def_names(
    db: &AnalysisContext<'_>,
    ident: &hir_def::Ident,
    defs: impl IntoIterator<Item = DefId>,
    names: &mut BTreeMap<String, NameKind>,
) {
    let defs = defs.into_iter().collect::<Vec<_>>();

    let subroutines = Resolution::from_candidates(
        defs.iter().filter_map(|def_id| def_id.primary_origin(db.db).as_subroutine(db.db)),
    );
    let return_ty = match subroutines {
        Resolution::Unresolved => None,
        Resolution::Unique(subroutine_id) => Some(subroutine_return_ty(db, subroutine_id)),
        Resolution::Ambiguous(_) => Some(Type::unknown()),
    };
    if let Some(return_ty) = return_ty {
        names.entry(ident.to_string()).or_insert(NameKind::SubroutineCall { return_ty });
        return;
    }

    if defs.iter().any(|def_id| {
        matches!(
            def_id.kind(db.db),
            DefKind::Variable
                | DefKind::Net
                | DefKind::Param
                | DefKind::Port
                | DefKind::Genvar
                | DefKind::Specparam
                | DefKind::SubroutinePort
        )
    }) {
        let res = Resolution::from_candidates(defs.iter().cloned());
        let ty = TypeSystem::new(db.db).type_of_resolution(res);
        names.entry(ident.to_string()).or_insert(NameKind::Value { ty });
    }
}
fn subroutine_return_ty(db: &AnalysisContext<'_>, subroutine: OwnerId) -> Type {
    TypeSystem::new(db.db).type_of_subroutine_return(subroutine)
}

fn module_id_for_container(db: &AnalysisContext<'_>, owner: OwnerId) -> Option<OwnerId> {
    ScopeParent::start_from(db.db, owner).find(|owner| owner.kind(db.db) == OwnerKind::Module)
}
fn expression_candidate_matches_expected_type(
    db: &AnalysisContext<'_>,
    expected_ty: Option<&Type>,
    kind: &NameKind,
) -> bool {
    let Some(expected_ty) = expected_ty else {
        return true;
    };
    let candidate_ty = match kind {
        NameKind::Value { ty } => ty,
        NameKind::SubroutineCall { return_ty } => return_ty,
    };
    is_compatible_typed_value(db, expected_ty, candidate_ty)
}

fn expected_type_at_offset(
    db: &AnalysisContext<'_>,
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    root: SyntaxNode<'_>,
    offset: TextSize,
    _current_module_id: OwnerId,
) -> Option<Type> {
    expected_type_for_assignment_rhs(db, sema, file_id, root, offset)
        .or_else(|| expected_type_for_declarator_initializer(db, sema, file_id, root, offset))
        .filter(|ty| TypeSystem::new(db.db).is_typed_value(ty))
}

fn expected_type_for_assignment_rhs(
    db: &AnalysisContext<'_>,
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    root: SyntaxNode<'_>,
    offset: TextSize,
) -> Option<Type> {
    let assignment = root.find_node_at_offset::<ast::BinaryExpression<'_>>(offset)?;
    if !is_assignment_expression(assignment.syntax().kind()) {
        return None;
    }
    let right = assignment.right();
    if !right.syntax().text_range().is_some_and(|range| {
        range.contains(offset) || range.start() == offset || range.end() == offset
    }) {
        return None;
    }

    let res = sema.expr_to_def(sema.resolve_expr(file_id, assignment.left())?);
    Some(TypeSystem::new(db.db).type_of_resolution(res))
}

fn expected_type_for_declarator_initializer(
    db: &AnalysisContext<'_>,
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    root: SyntaxNode<'_>,
    offset: TextSize,
) -> Option<Type> {
    let declarator = root.find_node_at_offset::<ast::Declarator<'_>>(offset)?;
    let initializer = declarator.initializer()?;
    if !initializer.expr().syntax().text_range().is_some_and(|range| {
        range.contains(offset) || range.start() == offset || range.end() == offset
    }) {
        return None;
    }

    let ident = lower_ident_opt(declarator.name())?;
    let container_id = sema.container_for_node(file_id, declarator.syntax())?;
    let res = sema.name_to_def(OwnerRef::new(container_id, ident));
    Some(TypeSystem::new(db.db).type_of_resolution(res))
}

fn is_assignment_expression(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::ASSIGNMENT_EXPRESSION
            | SyntaxKind::NONBLOCKING_ASSIGNMENT_EXPRESSION
            | SyntaxKind::ADD_ASSIGNMENT_EXPRESSION
            | SyntaxKind::SUBTRACT_ASSIGNMENT_EXPRESSION
            | SyntaxKind::MULTIPLY_ASSIGNMENT_EXPRESSION
            | SyntaxKind::DIVIDE_ASSIGNMENT_EXPRESSION
            | SyntaxKind::MOD_ASSIGNMENT_EXPRESSION
            | SyntaxKind::AND_ASSIGNMENT_EXPRESSION
            | SyntaxKind::OR_ASSIGNMENT_EXPRESSION
            | SyntaxKind::XOR_ASSIGNMENT_EXPRESSION
            | SyntaxKind::LOGICAL_LEFT_SHIFT_ASSIGNMENT_EXPRESSION
            | SyntaxKind::LOGICAL_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION
            | SyntaxKind::ARITHMETIC_LEFT_SHIFT_ASSIGNMENT_EXPRESSION
            | SyntaxKind::ARITHMETIC_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION
    )
}
