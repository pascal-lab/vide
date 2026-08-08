use hir_def::{
    Ident,
    container::{InFile, OwnerRef},
    db::HirDefDb,
    def_id::DefId,
    expr::{Expr, ExprId},
    owner::OwnerId,
    pathres::{
        NameRef, RefKind, resolve_child_name, resolve_name, resolve_name_at, resolve_path_at,
    },
    symbol::{NameContext, Resolution},
};

pub(super) fn expr_to_def(
    db: &dyn HirDefDb,
    OwnerRef { cont_id, value: expr_id }: OwnerRef<ExprId>,
) -> Resolution<DefId> {
    // Expression references resolve at their source position; call callees
    // additionally search every scope to its end (IEEE 1800-2017 26.3).
    let reference = expr_reference(db, cont_id, expr_id);
    let resolve = |expr: &Expr| match expr {
        Expr::Field { receiver, field } => {
            let Some(field) = field.as_ref() else {
                return Resolution::Unresolved;
            };
            resolve_expr_path(db, cont_id, expr_id, NameContext::Value, reference.as_ref()).or_else(
                || {
                    let receiver_res = expr_to_def(db, OwnerRef::new(cont_id, *receiver));
                    resolve_child_name(db, &receiver_res, field, NameContext::Value)
                },
            )
        }
        Expr::ElementSelect { receiver, .. } => {
            resolve_expr_path(db, cont_id, expr_id, NameContext::Value, reference.as_ref())
                .or_else(|| expr_to_def(db, OwnerRef::new(cont_id, *receiver)))
        }
        Expr::Ident(ident) => name_to_def_at(
            db,
            OwnerRef::new(cont_id, ident.clone()),
            NameContext::Value,
            reference.as_ref(),
        ),
        _ => Resolution::Unresolved,
    };

    let Some(expr) = expr_in_container(db, cont_id, expr_id) else {
        return Resolution::Unresolved;
    };
    resolve(&expr)
}

/// Reference position of an expression, derived from its canonical source.
/// An expression that is exactly the callee of some `Call` resolves as a
/// call reference (searches to the end of each scope).
fn expr_reference(db: &dyn HirDefDb, cont_id: OwnerId, expr_id: ExprId) -> Option<NameRef> {
    let file_id = cont_id.file(db);
    let source = db.body_with_source_map(cont_id).source_map().expr_srcs.hir_to_src(expr_id)?;
    let is_callee = db
        .body_with_source_map(cont_id)
        .data_ref()
        .exprs
        .iter()
        .any(|(_, expr)| matches!(expr, Expr::Call { callee, .. } if *callee == expr_id));
    let kind = if is_callee { RefKind::Call } else { RefKind::Value };
    Some(NameRef { position: InFile::new(file_id, source), kind })
}

pub(super) fn name_to_def(
    db: &dyn HirDefDb,
    OwnerRef { cont_id, value: ident }: OwnerRef<Ident>,
    name_ctx: NameContext,
) -> Resolution<DefId> {
    resolve_name(db, cont_id, &ident, name_ctx)
}

pub(super) fn name_to_def_at(
    db: &dyn HirDefDb,
    OwnerRef { cont_id, value: ident }: OwnerRef<Ident>,
    name_ctx: NameContext,
    reference: Option<&hir_def::pathres::NameRef>,
) -> Resolution<DefId> {
    resolve_name_at(db, cont_id, &ident, name_ctx, reference)
}

fn resolve_expr_path(
    db: &dyn HirDefDb,
    cont_id: OwnerId,
    expr_id: ExprId,
    ctx: NameContext,
    reference: Option<&NameRef>,
) -> Resolution<DefId> {
    let Some(path) = expr_path(db, cont_id, expr_id) else {
        return Resolution::Unresolved;
    };
    resolve_path_at(db, cont_id, &path, ctx, reference)
}

fn expr_path(db: &dyn HirDefDb, cont_id: OwnerId, expr_id: ExprId) -> Option<Vec<Ident>> {
    match expr_in_container(db, cont_id, expr_id)? {
        Expr::Ident(ident) => Some(vec![ident]),
        Expr::Field { receiver, field } => {
            let mut path = expr_path(db, cont_id, receiver)?;
            path.push(field?);
            Some(path)
        }
        Expr::ElementSelect { receiver, .. } => expr_path(db, cont_id, receiver),
        _ => None,
    }
}

fn expr_in_container(db: &dyn HirDefDb, cont_id: OwnerId, expr_id: ExprId) -> Option<Expr> {
    let container = cont_id.data(db);
    Some(container.expr(expr_id).clone())
}
