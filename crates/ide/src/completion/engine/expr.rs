use std::collections::BTreeMap;

use hir::{
    container::{ContainerId, ContainerParent, InContainer, InSubroutine},
    db::HirDb,
    file::HirFileId,
    hir_def::{
        lower_ident_opt,
        module::ModuleId,
        subroutine::{SubroutineId, SubroutineKind},
    },
    scope::{
        AnsiPortEntry, BlockEntry, GenerateBlockEntry, ModuleEntry, NonAnsiPortEntry,
        SubroutineEntry, UnitEntry,
    },
    semantics::{Semantics, pathres::PathResolution},
    type_infer::{Ty, normalize_data_ty, type_class, type_of_decl, type_of_path_resolution},
};
use syntax::{
    SyntaxKind, SyntaxNode, SyntaxNodeExt,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};
use utils::text_edit::TextSize;

use super::{candidate::CompletionCandidate, system, typed_filter::is_compatible_typed_value};
use crate::{FilePosition, completion::context::CompletionContext, db::root_db::RootDb};

#[derive(Clone, Debug)]
enum NameKind {
    Value { ty: Ty },
    SubroutineCall { return_ty: Ty },
}

pub(super) fn complete_expression(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
    complete_expression_impl(db, position, prefix, ctx)
}

pub(super) fn complete_argument_exprs(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
    complete_expression_impl(db, position, prefix, ctx)
}

fn complete_expression_impl(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
    let sema = Semantics::new(db);
    let file_id = position.file_id.into();
    let parsed_file = sema.parse_file(position.file_id);
    let Some(root) = parsed_file.root() else {
        return Vec::new();
    };

    let mut names: BTreeMap<String, NameKind> = BTreeMap::new();
    let mut current_module_id = None;

    if let Some(container_id) = container_id_at_offset(&sema, file_id, root, position.offset) {
        current_module_id = module_id_for_container(db, container_id);
        for container_id in ContainerParent::start_from(db, container_id) {
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
) -> Option<ContainerId> {
    let elem = root.covering_element(utils::line_index::TextRange::empty(offset));
    let node = elem.as_node().or_else(|| elem.parent())?;
    sema.container_for_node(file_id, node)
}

fn collect_container_names(
    db: &RootDb,
    container_id: ContainerId,
    names: &mut BTreeMap<String, NameKind>,
) {
    match container_id {
        ContainerId::HirFileId(file_id) => collect_file_names(db, file_id, names),
        ContainerId::ModuleId(module_id) => collect_module_names(db, module_id, names),
        ContainerId::GenerateBlockId(generate_block_id) => {
            let scope = db.generate_block_scope(generate_block_id);
            for (ident, entry) in scope.iter() {
                match entry {
                    GenerateBlockEntry::DeclId(decl_id) => {
                        names.entry(ident.to_string()).or_insert(NameKind::Value {
                            ty: type_of_decl(db, InContainer::new(container_id, decl_id)).ty,
                        });
                    }
                    GenerateBlockEntry::SubroutineId(subroutine_id) => {
                        names.entry(ident.to_string()).or_insert(NameKind::SubroutineCall {
                            return_ty: subroutine_return_ty(db, subroutine_id),
                        });
                    }
                    _ => {}
                }
            }
        }
        ContainerId::BlockId(block_id) => {
            let scope = db.block_scope(block_id);
            for (ident, entry) in scope.iter() {
                if let BlockEntry::DeclId(decl_id) = entry {
                    names.entry(ident.to_string()).or_insert(NameKind::Value {
                        ty: type_of_decl(db, InContainer::new(container_id, decl_id)).ty,
                    });
                }
            }
        }
        ContainerId::SubroutineId(subroutine_id) => {
            let scope = db.subroutine_scope(subroutine_id);
            for (ident, entry) in scope.iter() {
                match entry {
                    SubroutineEntry::DeclId(decl_id) => {
                        names.entry(ident.to_string()).or_insert(NameKind::Value {
                            ty: type_of_decl(db, InContainer::new(container_id, decl_id)).ty,
                        });
                    }
                    SubroutineEntry::SubroutinePortId(port_id) => {
                        let ty = type_of_path_resolution(
                            db,
                            PathResolution::SubroutinePort(InSubroutine::new(
                                subroutine_id,
                                port_id,
                            )),
                        )
                        .ty;
                        names.entry(ident.to_string()).or_insert(NameKind::Value { ty });
                    }
                    _ => {}
                }
            }
        }
    }
}

fn collect_file_names(db: &RootDb, file_id: HirFileId, names: &mut BTreeMap<String, NameKind>) {
    let scope = db.file_scope(file_id);
    for (ident, entry) in scope.iter() {
        if let UnitEntry::FiledDeclId(decl_id) = entry {
            names.entry(ident.to_string()).or_insert(NameKind::Value {
                ty: type_of_decl(
                    db,
                    InContainer::new(ContainerId::HirFileId(file_id), decl_id.value),
                )
                .ty,
            });
        }
    }
}

fn collect_module_names(db: &RootDb, module_id: ModuleId, names: &mut BTreeMap<String, NameKind>) {
    let scope = db.module_scope(module_id);
    for (ident, entry) in scope.iter() {
        match entry {
            ModuleEntry::DeclId(decl_id) => {
                names.entry(ident.to_string()).or_insert(NameKind::Value {
                    ty: type_of_decl(db, InContainer::new(module_id.into(), decl_id)).ty,
                });
            }
            ModuleEntry::AnsiPortEntry(AnsiPortEntry(decl_id)) => {
                names.entry(ident.to_string()).or_insert(NameKind::Value {
                    ty: type_of_decl(db, InContainer::new(module_id.into(), decl_id)).ty,
                });
            }
            ModuleEntry::NonAnsiPortEntry(NonAnsiPortEntry { port_decl, data_decl, .. }) => {
                let ty = data_decl
                    .or(port_decl)
                    .map(|decl_id| type_of_decl(db, InContainer::new(module_id.into(), decl_id)).ty)
                    .unwrap_or(Ty::Unknown);
                names.entry(ident.to_string()).or_insert(NameKind::Value { ty });
            }
            ModuleEntry::SubroutineId(subroutine_id) => {
                names.entry(ident.to_string()).or_insert(NameKind::SubroutineCall {
                    return_ty: subroutine_return_ty(db, subroutine_id),
                });
            }
            _ => {}
        }
    }
}

fn subroutine_return_ty(db: &RootDb, subroutine_id: SubroutineId) -> Ty {
    match db.subroutine(subroutine_id).kind {
        SubroutineKind::Function { return_ty: Some(return_ty) } => {
            normalize_data_ty(db, ContainerId::SubroutineId(subroutine_id), return_ty).ty
        }
        SubroutineKind::Function { return_ty: None } | SubroutineKind::Task => Ty::Unknown,
    }
}

fn module_id_for_container(db: &RootDb, container_id: ContainerId) -> Option<ModuleId> {
    ContainerParent::start_from(db, container_id).find_map(|container_id| match container_id {
        ContainerId::ModuleId(module_id) => Some(module_id),
        _ => None,
    })
}

fn expression_candidate_matches_expected_type(
    db: &RootDb,
    expected_ty: Option<&Ty>,
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
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    root: SyntaxNode<'_>,
    offset: TextSize,
    _current_module_id: ModuleId,
) -> Option<Ty> {
    expected_type_for_assignment_rhs(db, sema, file_id, root, offset)
        .or_else(|| expected_type_for_declarator_initializer(db, sema, file_id, root, offset))
        .filter(|ty| type_class(db, ty).is_some())
}

fn expected_type_for_assignment_rhs(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    root: SyntaxNode<'_>,
    offset: TextSize,
) -> Option<Ty> {
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

    let res = sema.expr_to_def(sema.resolve_expr(file_id, assignment.left())?)?;
    Some(type_of_path_resolution(db, res).ty)
}

fn expected_type_for_declarator_initializer(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    root: SyntaxNode<'_>,
    offset: TextSize,
) -> Option<Ty> {
    let declarator = root.find_node_at_offset::<ast::Declarator<'_>>(offset)?;
    let initializer = declarator.initializer()?;
    if !initializer.expr().syntax().text_range().is_some_and(|range| {
        range.contains(offset) || range.start() == offset || range.end() == offset
    }) {
        return None;
    }

    let ident = lower_ident_opt(declarator.name())?;
    let container_id = sema.container_for_node(file_id, declarator.syntax())?;
    let res = sema.name_to_def(InContainer::new(container_id, ident))?;
    Some(type_of_path_resolution(db, res).ty)
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
