use hir::{
    db::{HirDb, InternDb},
    hir_def::{
        Ident,
        declaration::Declaration,
        expr::{
            BinaryOp, Expr, ExprId, UnaryOp,
            data_ty::{BuiltinDataTy, DataTy, Dimension, IntKind},
        },
        literal::Literal,
        module::ModuleId,
    },
    scope::ModuleEntry,
    semantics::Semantics,
};
use ide_db::root_db::RootDb;
use span::FilePosition;
use syntax::ast::{self, AstNode};
use utils::{
    get::{Get, GetRef},
    text_edit::{TextEditItem, TextRange},
};

use crate::completion::context::{
    CompletionContext, DotKind, LexContext, Qualifier, TriggerChar, completion_context,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub edit: Option<TextEditItem>,
}

pub fn completions(
    db: &RootDb,
    position: FilePosition,
    trigger: Option<TriggerChar>,
) -> Vec<CompletionItem> {
    let ctx = completion_context(db, position, trigger);
    completions_with_context(db, position, &ctx)
}

fn completions_with_context(
    db: &RootDb,
    position: FilePosition,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    if ctx.lex != LexContext::Code {
        return Vec::new();
    }

    match ctx.qualifier {
        Some(Qualifier::AfterDot(after_dot)) => match after_dot.kind {
            DotKind::NamedPort => {
                complete_named_port_names(db, position, &ctx.prefix, ctx.replacement)
            }
            DotKind::NamedParam => {
                complete_named_param_names(db, position, &ctx.prefix, ctx.replacement)
            }
            DotKind::Member => Vec::new(),
        },
        Some(Qualifier::InNamedPortConnExpr) => {
            complete_named_port_conn_expr(db, position, &ctx.prefix, ctx.replacement)
        }
        Some(Qualifier::InNamedParamAssignExpr) => {
            complete_named_param_assign_expr(db, position, &ctx.prefix, ctx.replacement)
        }
        Some(Qualifier::AfterHash(_)) => Vec::new(),
        Some(Qualifier::InParenList(_)) => Vec::new(),
        Some(Qualifier::AfterAt(_)) => Vec::new(),
        Some(Qualifier::AfterBacktick) => Vec::new(),
        None => Vec::new(),
    }
}

fn complete_named_port_conn_expr(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    replacement: TextRange,
) -> Vec<CompletionItem> {
    let sema = Semantics::new(db);
    let file = sema.parse(position.file_id);
    let Some(conn) =
        sema.find_node_at_offset::<ast::NamedPortConnection>(file.syntax(), position.offset)
    else {
        return Vec::new();
    };

    let Some(port_name) = hir::hir_def::lower_ident_opt(conn.name()) else {
        return Vec::new();
    };

    let Some(instantiation) = enclosing_instantiation(conn.syntax()) else {
        return Vec::new();
    };

    let current_module_id = sema.resolve_instantiation(instantiation).module_id;
    let Some(target_module_id) = sema.nameres_instantiation(instantiation) else {
        return Vec::new();
    };

    let target_module = db.module(target_module_id);
    let expected_ty = expected_port_ty(db, &target_module, target_module_id, &port_name);

    let current_module = db.module(current_module_id);
    let candidates = value_candidates_in_module(db, current_module_id);

    candidates
        .into_iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .filter(|(_, candidate_ty)| {
            expected_ty.is_none_or(|expected_ty| {
                is_compatible_typed_value(
                    db,
                    &target_module,
                    expected_ty,
                    &current_module,
                    *candidate_ty,
                )
            })
        })
        .map(|(name, _)| CompletionItem {
            label: name.clone(),
            edit: Some(TextEditItem::replace(replacement, name)),
        })
        .collect()
}

fn complete_named_param_assign_expr(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    replacement: TextRange,
) -> Vec<CompletionItem> {
    let sema = Semantics::new(db);
    let file = sema.parse(position.file_id);
    let Some(assign) =
        sema.find_node_at_offset::<ast::NamedParamAssignment>(file.syntax(), position.offset)
    else {
        return Vec::new();
    };

    let Some(param_name) = hir::hir_def::lower_ident_opt(assign.name()) else {
        return Vec::new();
    };

    let Some(instantiation) = enclosing_instantiation(assign.syntax()) else {
        return Vec::new();
    };

    let current_module_id = sema.resolve_instantiation(instantiation).module_id;
    let Some(target_module_id) = sema.nameres_instantiation(instantiation) else {
        return Vec::new();
    };

    let target_module = db.module(target_module_id);
    let expected_ty = expected_param_ty(db, &target_module, target_module_id, &param_name);

    let current_module = db.module(current_module_id);
    let candidates = const_candidates_in_module(db, current_module_id);

    candidates
        .into_iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .filter(|(_, candidate_ty)| {
            expected_ty.is_none_or(|expected_ty| {
                is_compatible_typed_value(
                    db,
                    &target_module,
                    expected_ty,
                    &current_module,
                    *candidate_ty,
                )
            })
        })
        .map(|(name, _)| CompletionItem {
            label: name.clone(),
            edit: Some(TextEditItem::replace(replacement, name)),
        })
        .collect()
}

fn complete_named_port_names(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    replacement: TextRange,
) -> Vec<CompletionItem> {
    let sema = Semantics::new(db);
    let file = sema.parse(position.file_id);
    let Some(instantiation) =
        sema.find_node_at_offset::<ast::HierarchyInstantiation>(file.syntax(), position.offset)
    else {
        return Vec::new();
    };
    let Some(target_module_id) = sema.nameres_instantiation(instantiation) else {
        return Vec::new();
    };

    ports_of_module(db, target_module_id)
        .into_iter()
        .filter(|name| name.starts_with(prefix))
        .map(|name| CompletionItem {
            label: name.clone(),
            edit: Some(TextEditItem::replace(replacement, name)),
        })
        .collect()
}

fn complete_named_param_names(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    replacement: TextRange,
) -> Vec<CompletionItem> {
    let sema = Semantics::new(db);
    let file = sema.parse(position.file_id);
    let Some(instantiation) =
        sema.find_node_at_offset::<ast::HierarchyInstantiation>(file.syntax(), position.offset)
    else {
        return Vec::new();
    };
    let Some(target_module_id) = sema.nameres_instantiation(instantiation) else {
        return Vec::new();
    };

    overridable_params_of_module(db, target_module_id)
        .into_iter()
        .filter(|name| name.starts_with(prefix))
        .map(|name| CompletionItem {
            label: name.clone(),
            edit: Some(TextEditItem::replace(replacement, name)),
        })
        .collect()
}

fn ports_of_module(db: &RootDb, module_id: ModuleId) -> Vec<String> {
    let module = db.module(module_id);
    let mut names = Vec::new();

    match &module.ports {
        hir::hir_def::module::port::Ports::Ansi(port_decls) => {
            for (_, port_decl) in port_decls.iter() {
                for decl_id in port_decl.decls.clone() {
                    if let Some(name) = module.get(decl_id).name.as_ref() {
                        names.push(name.to_string());
                    }
                }
            }
        }
        hir::hir_def::module::port::Ports::NonAnsi { ports, .. } => {
            for (_, port) in ports.iter() {
                if let Some(label) = port.label.as_ref() {
                    names.push(label.to_string());
                }
            }
        }
    }

    names.sort();
    names.dedup();
    names
}

fn overridable_params_of_module(db: &RootDb, module_id: ModuleId) -> Vec<String> {
    let (module, module_src_map) = db.module_with_source_map(module_id);
    let tree = db.parse(module_id.file_id);

    let mut names = Vec::new();

    for (_decl_id, decl) in module.decls.iter() {
        if decl.name.is_none() {
            continue;
        }
        let hir::hir_def::expr::declarator::DeclaratorParent::DeclarationId(declaration_id) =
            decl.parent
        else {
            continue;
        };
        let hir::hir_def::declaration::Declaration::ParamDecl(_) = module.get(declaration_id)
        else {
            continue;
        };

        let src = module_src_map.get(declaration_id);
        let hir::hir_def::declaration::DeclarationSrc::ParameterDeclaration(ptr) = src else {
            continue;
        };
        let Some(ast_decl) = ptr.to_node(&tree).and_then(ast::ParameterDeclaration::cast) else {
            continue;
        };

        let Some(keyword) = ast_decl.keyword() else {
            continue;
        };
        if keyword.kind() != syntax::Token![parameter] {
            continue;
        }

        names.push(decl.name.as_ref().unwrap().to_string());
    }

    names.sort();
    names.dedup();
    names
}

fn enclosing_instantiation(node: syntax::SyntaxNode) -> Option<ast::HierarchyInstantiation> {
    syntax::SyntaxAncestors::start_from(node).find_map(ast::HierarchyInstantiation::cast)
}

fn expected_port_ty(
    db: &RootDb,
    target_module: &hir::hir_def::module::Module,
    target_module_id: ModuleId,
    port_name: &Ident,
) -> Option<DataTy> {
    let scope = db.module_scope(target_module_id);
    let entry = scope.get(port_name)?;

    match entry {
        ModuleEntry::AnsiPortEntry(hir::scope::AnsiPortEntry(decl_id)) => {
            decl_ty_in_module(target_module, decl_id)
        }
        ModuleEntry::NonAnsiPortEntry(entry) => {
            let decl_id = entry.port_decl.or(entry.data_decl)?;
            decl_ty_in_module(target_module, decl_id)
        }
        _ => None,
    }
}

fn expected_param_ty(
    db: &RootDb,
    target_module: &hir::hir_def::module::Module,
    target_module_id: ModuleId,
    param_name: &Ident,
) -> Option<DataTy> {
    let scope = db.module_scope(target_module_id);
    let ModuleEntry::DeclId(decl_id) = scope.get(param_name)? else {
        return None;
    };

    let hir::hir_def::expr::declarator::DeclaratorParent::DeclarationId(declaration_id) =
        target_module.get(decl_id).parent
    else {
        return None;
    };
    let Declaration::ParamDecl(param_decl) = target_module.get(declaration_id) else {
        return None;
    };

    is_overridable_parameter_decl(db, target_module_id, declaration_id).then_some(param_decl.ty)
}

fn is_overridable_parameter_decl(
    db: &RootDb,
    module_id: ModuleId,
    declaration_id: hir::hir_def::declaration::DeclarationId,
) -> bool {
    let (_, module_src_map) = db.module_with_source_map(module_id);
    let tree = db.parse(module_id.file_id);
    let src = module_src_map.get(declaration_id);
    let hir::hir_def::declaration::DeclarationSrc::ParameterDeclaration(ptr) = src else {
        return false;
    };
    let Some(node) = ptr.to_node(&tree) else {
        return false;
    };

    node.first_token().is_some_and(|kw| kw.kind() == syntax::Token![parameter])
}

fn decl_ty_in_module(
    module: &hir::hir_def::module::Module,
    decl_id: hir::hir_def::expr::declarator::DeclId,
) -> Option<DataTy> {
    use hir::hir_def::expr::declarator::DeclaratorParent;
    match module.get(decl_id).parent {
        DeclaratorParent::PortDeclId(port_decl_id) => {
            Some(module.ports.get(port_decl_id).header.ty())
        }
        DeclaratorParent::DeclarationId(declaration_id) => Some(module.get(declaration_id).ty()),
        DeclaratorParent::StmtId(_) => None,
    }
}

fn value_candidates_in_module(db: &RootDb, module_id: ModuleId) -> Vec<(String, DataTy)> {
    let module = db.module(module_id);
    let mut candidates: Vec<(String, DataTy)> = Vec::new();

    for (_, decl) in module.declarations.iter() {
        let ty = decl.ty();
        match decl {
            Declaration::DataDecl(_) | Declaration::NetDecl(_) => {
                for decl_id in decl.decls().clone() {
                    if let Some(name) = module.get(decl_id).name.as_ref() {
                        candidates.push((name.to_string(), ty));
                    }
                }
            }
            Declaration::ParamDecl(_) => {}
        }
    }

    match &module.ports {
        hir::hir_def::module::port::Ports::Ansi(port_decls) => {
            for (_, port_decl) in port_decls.iter() {
                let ty = port_decl.header.ty();
                for decl_id in port_decl.decls.clone() {
                    if let Some(name) = module.get(decl_id).name.as_ref() {
                        candidates.push((name.to_string(), ty));
                    }
                }
            }
        }
        hir::hir_def::module::port::Ports::NonAnsi { decls, .. } => {
            for (_, port_decl) in decls.iter() {
                let ty = port_decl.header.ty();
                for decl_id in port_decl.decls.clone() {
                    if let Some(name) = module.get(decl_id).name.as_ref() {
                        candidates.push((name.to_string(), ty));
                    }
                }
            }
        }
    }

    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    candidates.dedup_by(|a, b| a.0 == b.0);
    candidates
}

fn const_candidates_in_module(db: &RootDb, module_id: ModuleId) -> Vec<(String, DataTy)> {
    let module = db.module(module_id);
    let mut candidates: Vec<(String, DataTy)> = Vec::new();

    for (_, decl) in module.declarations.iter() {
        let Declaration::ParamDecl(param_decl) = decl else {
            continue;
        };
        for decl_id in param_decl.decls.clone() {
            if let Some(name) = module.get(decl_id).name.as_ref() {
                candidates.push((name.to_string(), param_decl.ty));
            }
        }
    }

    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    candidates.dedup_by(|a, b| a.0 == b.0);
    candidates
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TyClass {
    Integral,
    Real,
    String,
}

fn type_class(db: &RootDb, ty: DataTy) -> Option<TyClass> {
    let DataTy::Builtin(id) = ty else {
        return None;
    };
    match db.lookup_intern_ty(id) {
        BuiltinDataTy::Int { .. } | BuiltinDataTy::Vector { .. } => Some(TyClass::Integral),
        BuiltinDataTy::Real(_) => Some(TyClass::Real),
        BuiltinDataTy::String => Some(TyClass::String),
    }
}

fn is_compatible_typed_value(
    db: &RootDb,
    expected_module: &hir::hir_def::module::Module,
    expected_ty: DataTy,
    candidate_module: &hir::hir_def::module::Module,
    candidate_ty: DataTy,
) -> bool {
    let (Some(expected_class), Some(candidate_class)) =
        (type_class(db, expected_ty), type_class(db, candidate_ty))
    else {
        return true;
    };
    if expected_class != candidate_class {
        return false;
    }

    if expected_class != TyClass::Integral {
        return true;
    }

    let expected_w = packed_bit_width(db, expected_module, expected_ty);
    let candidate_w = packed_bit_width(db, candidate_module, candidate_ty);
    match (expected_w, candidate_w) {
        (Some(a), Some(b)) => a == b,
        _ => true,
    }
}

fn packed_bit_width(db: &RootDb, module: &hir::hir_def::module::Module, ty: DataTy) -> Option<u64> {
    let DataTy::Builtin(id) = ty else {
        return None;
    };
    let builtin = db.lookup_intern_ty(id);
    match builtin {
        BuiltinDataTy::String | BuiltinDataTy::Real(_) => None,
        BuiltinDataTy::Int { kind, .. } => Some(int_kind_width(kind) as u64),
        BuiltinDataTy::Vector { dimensions, .. } => {
            if dimensions.is_empty() {
                return Some(1);
            }

            let mut product: u64 = 1;
            for dim in dimensions {
                let dim = dim?;
                let width = match dim {
                    Dimension::Range(left, right) => {
                        let l = eval_const_i128(module, left, db)?;
                        let r = eval_const_i128(module, right, db)?;
                        i128::abs(l - r).checked_add(1)?
                    }
                    Dimension::Size(size) => eval_const_i128(module, size, db)?,
                };
                let width: u64 = width.try_into().ok()?;
                product = product.checked_mul(width)?;
            }
            Some(product)
        }
    }
}

fn int_kind_width(kind: IntKind) -> usize {
    match kind {
        IntKind::Byte => 8,
        IntKind::ShortInt => 16,
        IntKind::Int => 32,
        IntKind::LongInt => 64,
        IntKind::Integer => 32,
        IntKind::Time => 64,
    }
}

fn eval_const_i128(
    module: &hir::hir_def::module::Module,
    expr_id: ExprId,
    db: &RootDb,
) -> Option<i128> {
    match module.get(expr_id) {
        Expr::Literal(Literal::Int(int)) => int.get_single_word().map(|v| v as i128),
        Expr::Unary { op, expr } => {
            let v = eval_const_i128(module, *expr, db)?;
            match op {
                UnaryOp::Pos => Some(v),
                UnaryOp::Neg => Some(v.checked_neg()?),
                _ => None,
            }
        }
        Expr::Binary { op, lhs, rhs } => {
            let l = eval_const_i128(module, *lhs, db)?;
            let r = eval_const_i128(module, *rhs, db)?;
            match op {
                BinaryOp::Add => l.checked_add(r),
                BinaryOp::Sub => l.checked_sub(r),
                BinaryOp::Mul => l.checked_mul(r),
                BinaryOp::Div => (r != 0).then(|| l.checked_div(r)).flatten(),
                BinaryOp::Mod => (r != 0).then(|| l.checked_rem(r)).flatten(),
                BinaryOp::ShiftLeft => u32::try_from(r).ok().and_then(|s| l.checked_shl(s)),
                BinaryOp::ShiftRight => u32::try_from(r).ok().and_then(|s| l.checked_shr(s)),
                _ => None,
            }
        }
        Expr::Cast { expr, .. } | Expr::SignedCast { expr, .. } => {
            eval_const_i128(module, *expr, db)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use base_db::{change::Change, source_root::SourceRoot};
    use triomphe::Arc;
    use utils::{lines::LineEnding, text_edit::TextSize};
    use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

    use super::*;
    use crate::analysis_host::AnalysisHost;

    fn setup(text: &str) -> (AnalysisHost, FilePosition) {
        let marker = "/*caret*/";
        let off = text.find(marker).expect("missing /*caret*/");
        let mut owned = text.to_string();
        owned = owned.replace(marker, "");

        let file_id = FileId(0);
        let path = VfsPath::new_virtual_path("/test.v".to_string());

        let mut file_set = FileSet::default();
        file_set.insert(file_id, path);
        let root = SourceRoot::new_local(file_set);

        let mut change = Change::new();
        change.set_roots(vec![root]);
        change.add_changed_file(ChangedFile {
            file_id,
            change_kind: ChangeKind::Create(Arc::from(owned.as_str()), LineEnding::Unix),
        });

        let mut host = AnalysisHost::default();
        host.apply_change(change);
        let position = FilePosition { file_id, offset: TextSize::from(off as u32) };
        (host, position)
    }

    fn completions(text: &str) -> Vec<CompletionItem> {
        let (host, position) = setup(text);
        super::completions(host.raw_db(), position, None)
    }

    #[test]
    fn filters_named_port_connection_expr_by_width() {
        let items = completions(
            "module m(input [3:0] a); endmodule\n\
             module top;\n\
             wire [3:0] sig4;\n\
             wire [7:0] sig8;\n\
             wire sig1;\n\
             m u0(.a(/*caret*/));\n\
             endmodule\n",
        );
        let labels: Vec<_> = items.into_iter().map(|it| it.label).collect();
        assert!(labels.contains(&"sig4".to_string()));
        assert!(!labels.contains(&"sig8".to_string()));
        assert!(!labels.contains(&"sig1".to_string()));
    }

    #[test]
    fn filters_named_param_assign_expr_by_width() {
        let items = completions(
            "module m #(parameter [3:0] W = 4) (); endmodule\n\
             module top;\n\
             localparam [3:0] P4 = 4;\n\
             localparam [7:0] P8 = 8;\n\
             m #(.W(/*caret*/)) u0();\n\
             endmodule\n",
        );
        let labels: Vec<_> = items.into_iter().map(|it| it.label).collect();
        assert!(labels.contains(&"P4".to_string()));
        assert!(!labels.contains(&"P8".to_string()));
    }
}
