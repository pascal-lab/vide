use hir_def::{
    Ident,
    container::{ArenaOwnerId, InContainer},
    db::HirDefDb,
    def_id::DefId,
    expr::{Expr, ExprId},
    pathres::{resolve_child_name, resolve_name, resolve_path},
    symbol::{NameContext, Resolution},
};

pub(super) fn expr_to_def(
    db: &dyn HirDefDb,
    InContainer { cont_id, value: expr_id }: InContainer<ExprId>,
) -> Resolution<DefId> {
    let resolve = |expr: &Expr| {
        let res = match expr {
            Expr::Field { receiver, field } => {
                let Some(field) = field.as_ref() else {
                    return Resolution::Unresolved;
                };
                resolve_expr_path(db, cont_id, expr_id, NameContext::Value).or_else(|| {
                    let receiver_res = expr_to_def(db, InContainer::new(cont_id, *receiver));
                    resolve_child_name(db, &receiver_res, field, NameContext::Value)
                })
            }
            Expr::ElementSelect { receiver, .. } => resolve_expr_path(db, cont_id, expr_id, NameContext::Value)
                .or_else(|| expr_to_def(db, InContainer::new(cont_id, *receiver))),
            Expr::Ident(ident) => {
                name_to_def(db, InContainer::new(cont_id, ident.clone()), NameContext::Value)
            }
            _ => Resolution::Unresolved,
        };
        res
    };

    let Some(expr) = expr_in_container(db, cont_id, expr_id) else {
        return Resolution::Unresolved;
    };
    resolve(&expr)
}

pub(super) fn name_to_def(
    db: &dyn HirDefDb,
    InContainer { cont_id, value: ident }: InContainer<Ident>,
    name_ctx: NameContext,
) -> Resolution<DefId> {
    resolve_name(db, cont_id.into(), &ident, name_ctx)
}

fn resolve_expr_path(
    db: &dyn HirDefDb,
    cont_id: ArenaOwnerId,
    expr_id: ExprId,
    ctx: NameContext,
) -> Resolution<DefId> {
    let Some(path) = expr_path(db, cont_id, expr_id) else {
        return Resolution::Unresolved;
    };
    resolve_path(db, cont_id.into(), &path, ctx)
}

fn expr_path(db: &dyn HirDefDb, cont_id: ArenaOwnerId, expr_id: ExprId) -> Option<Vec<Ident>> {
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

fn expr_in_container(
    db: &dyn HirDefDb,
    cont_id: ArenaOwnerId,
    expr_id: ExprId,
) -> Option<Expr> {
    let container = cont_id.data(db);
    Some(container.expr(expr_id).clone())
}
