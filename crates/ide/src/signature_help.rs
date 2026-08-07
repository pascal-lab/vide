use std::collections::BTreeMap;

use hir_def::{
    container::{InContainer, InModule},
    declaration::Declaration,
    lower_ident_opt,
    module::{
        instantiation::{ParamAssign, PortConn},
        port::Ports,
    },
    subroutine::SubroutineKind,
    symbol::Resolution,
};
use hir_semantics::semantics::Semantics;
use hir_ty::display::HirDisplay;
use itertools::Either;
use preproc_expand::file::HirFileId;
use syntax::{
    SyntaxAncestors, SyntaxNodeExt,
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
    match_ast,
};
// Last week, I found an issue with the original strategy and have successfully implemented
// most of the intrinsic in LSV. and find some optimization opportunities. This week's goal is
// to pass lit tests in IV and migrate some optimizations.
use utils::text_edit::{TextRange, TextSize};

use crate::{
    FilePosition, db::root_db::RootDb, markup::Markup,
    module_resolution::resolve_instantiation_target,
};

#[derive(Debug)]
pub struct SignatureHelpConfig {
    pub params_only: bool,
}

#[derive(Debug)]
pub struct SignatureHelp {
    pub doc: Option<Markup>,
    pub label: String,
    pub active_parameter: Option<usize>,
    pub param_ranges: Vec<TextRange>,
    config: SignatureHelpConfig,
}

impl SignatureHelp {
    fn new(config: SignatureHelpConfig, label: String) -> Self {
        SignatureHelp { doc: None, label, active_parameter: None, param_ranges: Vec::new(), config }
    }

    fn push_param(&mut self, param: &str) {
        if !self.label.ends_with("(") {
            self.label.push_str(", ");
        }
        let start = TextSize::of(&self.label);
        self.label.push_str(param);
        let end = TextSize::of(&self.label);
        self.param_ranges.push(TextRange::new(start, end))
    }
}

pub(crate) fn signature_help(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
    config: SignatureHelpConfig,
) -> Option<SignatureHelp> {
    let sema = Semantics::new(db);
    let hir_file_id = file_id.into();
    let parsed_file = sema.parse_file(file_id);
    let root = parsed_file.root()?;
    let token = root.token_at_offset(offset).left_biased()?;

    for node in SyntaxAncestors::start_from(token.parent) {
        match_ast! { node,
            ast::HierarchicalInstance[it] => {
                if it.close_paren().is_none_or(|tok| tok != token.tok) {
                    return sig_help_for_instance(&sema, hir_file_id, it, offset, config);
                }
            },
            ast::HierarchyInstantiation[it] => {
                let Some(params) = it.parameters() else {
                    continue;
                };

                if params
                    .open_paren()
                    .and_then(|open_paren| open_paren.text_range_in(params.syntax()))
                    .is_some_and(|range| offset >= range.end())
                    && params
                        .close_paren()
                        .and_then(|close_paren| close_paren.text_range_in(params.syntax()))
                        .is_some_and(|range| offset <= range.start())
                {
                        return sig_help_for_instantiation(&sema, hir_file_id, it, offset, config);
                    }
            },
            ast::InvocationExpression[it] => {
                let Some(args) = it.arguments() else {
                    continue;
                };
                let in_args = args
                    .open_paren()
                    .and_then(|open_paren| open_paren.text_range_in(args.syntax()))
                    .is_some_and(|range| offset >= range.end())
                    && args.close_paren().is_none_or(|tok| tok != token.tok);
                if in_args {
                    return sig_help_for_invocation(&sema, hir_file_id, it, offset, config);
                }
            },
            _ => {},
        };
    }

    None
}

fn sig_help_for_instance(
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    instance: ast::HierarchicalInstance,
    offset: TextSize,
    config: SignatureHelpConfig,
) -> Option<SignatureHelp> {
    let db = sema.db;

    let active_param = 'blk: {
        let Some(InModule { value: instance_id, module_id }) =
            sema.resolve_instance(file_id, instance)
        else {
            break 'blk None;
        };
        let module = db.module_with_source_map(module_id);
        let instance = module.get(instance_id);

        let Some((idx, conn_id)) = instance.connections.iter().enumerate().find(|(_, conn_id)| {
            module.source_range(db, **conn_id).is_some_and(|range| range.end() >= offset)
        }) else {
            break 'blk None;
        };

        match module.get(*conn_id) {
            PortConn::Ordered(_) | PortConn::Empty => Some(Either::Left(idx)),
            PortConn::Named(name, _) if let Some(name) = name.as_ref() => {
                Some(Either::Right(name.to_owned()))
            }
            _ => None,
        }
    };

    let instantiation = ast::HierarchyInstantiation::cast(instance.syntax().parent()?)?;
    let target_module_id =
        resolve_instantiation_target(db, file_id.expect_file(), instantiation).unique()?;
    let target_module = db.module_with_source_map(target_module_id);
    let target_body = db.module_body_with_source_map(target_module_id);
    let target_module_name =
        target_module.name.as_ref().map(|name| name.to_string()).unwrap_or("<module>".to_string());

    let mut res = SignatureHelp::new(config, format!("module {target_module_name}("));

    if let Some(active_param) = &active_param {
        match active_param {
            Either::Left(idx) => res.active_parameter = Some(*idx),
            Either::Right(_) => {}
        }
    }

    match &target_module.ports {
        Ports::NonAnsi { ports, .. } => {
            let mut buf = String::new();
            for port in ports.values() {
                if let Some(label) = port.label.as_ref() {
                    buf.push_str(label.as_str());

                    if let Some(Either::Right(active_name)) = &active_param
                        && active_name == label.as_str()
                    {
                        res.active_parameter = Some(res.param_ranges.len() - 1);
                    }
                } else {
                    buf.push_str("<missing-label>");
                }

                buf.push('(');
                if let Some(refs) = &port.refs {
                    for r in refs.clone() {
                        let r = target_module.get(r);
                        buf.push_str(r.ident.as_ref().map(|s| s.as_str()).unwrap_or("<missing>"));
                        if let Some(select) = &r.select {
                            match InContainer::new(target_module_id.into(), *select)
                                .display_signature(db)
                            {
                                Ok(s) => buf.push_str(s.as_str()),
                                Err(_) => buf.push_str("<missing>"),
                            }
                        }
                    }
                }
                buf.push(')');
                res.push_param(buf.as_str());
            }
        }
        Ports::Ansi(port_decls) => {
            for port_decl in port_decls.values() {
                let mut buf = String::new();
                if !res.config.params_only {
                    let header = InModule::new(target_module_id, port_decl.header.clone())
                        .display_signature(db)
                        .unwrap_or_else(|_| "<missing-header>".to_string());
                    let header = header.trim_end();
                    buf.push_str(header);
                    if !header.is_empty() {
                        buf.push(' ');
                    }
                }
                let header_size = buf.len();

                for decl_id in port_decl.decls.clone() {
                    match InContainer::new(target_module_id.into(), decl_id).display_signature(db) {
                        Ok(decl) => buf.push_str(&decl),
                        Err(_) => buf.push_str("<missing>"),
                    }
                    res.push_param(buf.as_str());
                    buf.truncate(header_size);

                    if let Some(Either::Right(active_name)) = &active_param
                        && let Some(decl_name) = target_body.decls[decl_id].name.as_ref()
                        && active_name == decl_name.as_str()
                    {
                        res.active_parameter = Some(res.param_ranges.len() - 1);
                    }
                }
            }
        }
    };
    res.label.push(')');

    Some(res)
}

fn sig_help_for_instantiation(
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    instantiation: ast::HierarchyInstantiation,
    offset: TextSize,
    config: SignatureHelpConfig,
) -> Option<SignatureHelp> {
    let db = sema.db;

    let active_param = 'blk: {
        let Some(InModule { value: instantiation_id, module_id }) =
            sema.resolve_instantiation(file_id, instantiation)
        else {
            break 'blk None;
        };
        let module = db.module_with_source_map(module_id);
        let instantiation = module.get(instantiation_id);

        let Some((idx, conn_id)) =
            instantiation.param_assigns.iter().enumerate().find(|(_, conn_id)| {
                module.source_range(db, **conn_id).is_some_and(|range| range.end() >= offset)
            })
        else {
            break 'blk None;
        };

        match module.get(*conn_id) {
            ParamAssign::Ordered(_) => Some(Either::Left(idx)),
            ParamAssign::Named(name, _) if let Some(name) = name.as_ref() => {
                Some(Either::Right(name.to_owned()))
            }
            _ => None,
        }
    };

    let target_module_id =
        resolve_instantiation_target(db, file_id.expect_file(), instantiation).unique()?;
    let target_module = db.module_with_source_map(target_module_id);
    let target_body = db.module_body_with_source_map(target_module_id);
    let target_module_name =
        target_module.name.as_ref().map(|name| name.to_string()).unwrap_or("<module>".to_string());

    let mut res = SignatureHelp::new(config, format!("module {target_module_name} #("));

    if let Some(active_param) = &active_param {
        match active_param {
            Either::Left(idx) => res.active_parameter = Some(*idx),
            Either::Right(_) => {}
        }
    }

    for port_decl in target_body
        .declarations
        .values()
        .take_while(|declaration| matches!(declaration, Declaration::ParamDecl(_)))
        .filter(|declaration| {
            matches!(
                declaration,
                Declaration::ParamDecl(param_decl) if param_decl.kind.is_overridable()
            )
        })
    {
        let mut buf = String::new();
        if !res.config.params_only {
            let ty = InContainer::new(target_module_id.into(), port_decl.ty())
                .display_signature(db)
                .unwrap_or_default();
            buf.push_str(&ty);
            if !ty.is_empty() {
                buf.push(' ');
            }
        }
        let header_size = buf.len();

        for decl_id in port_decl.decls() {
            match InContainer::new(target_module_id.into(), decl_id).display_signature(db) {
                Ok(decl) => buf.push_str(&decl),
                Err(_) => buf.push_str("<missing>"),
            }
            res.push_param(buf.as_str());
            buf.truncate(header_size);

            if let Some(Either::Right(active_name)) = &active_param
                && let Some(decl_name) = target_body.decls[decl_id].name.as_ref()
                && active_name == decl_name.as_str()
            {
                res.active_parameter = Some(res.param_ranges.len() - 1);
            }
        }
    }

    res.label.push(')');
    Some(res)
}

fn sig_help_for_invocation(
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    invocation: ast::InvocationExpression,
    offset: TextSize,
    config: SignatureHelpConfig,
) -> Option<SignatureHelp> {
    let db = sema.db;

    // System subroutines are not name-resolvable; serve them from the built-in
    // signature table before falling back to scope resolution.
    if let Some(system_name) = system_identifier_of(invocation.left()) {
        return sig_help_for_system_call(&system_name, invocation, offset, config);
    }

    let callee = sema.resolve_expr(file_id, invocation.left())?;
    let subroutine_id = Resolution::from_candidates(
        sema.expr_to_def(callee)
            .candidates()
            .iter()
            .filter_map(|def_id| def_id.primary_origin(db).as_subroutine(db)),
    )
    .unique()?;
    let subroutine = db.subroutine(subroutine_id.clone());
    let subroutine_name = subroutine.name.as_ref()?;
    let container: hir_def::container::ArenaOwnerId = subroutine_id.cont_id.into();

    let active_param =
        invocation.arguments().and_then(|args| active_argument_at_offset(args, offset));

    let mut res = SignatureHelp::new(
        config,
        match &subroutine.kind {
            SubroutineKind::Task => format!("task {subroutine_name}("),
            SubroutineKind::Function { return_ty } => {
                let ty = return_ty.as_ref().and_then(|ty| {
                    InContainer::new(container.clone(), ty.clone()).display_source(db).ok()
                });
                match ty {
                    Some(ty) => format!("function {ty} {subroutine_name}("),
                    None => format!("function {subroutine_name}("),
                }
            }
        },
    );

    for (idx, port) in subroutine.ports.iter().enumerate() {
        let Some(port_name) = port.name.as_ref() else {
            continue;
        };

        let mut param = String::new();
        if !res.config.params_only {
            let ty = port.ty.as_ref().and_then(|ty| {
                InContainer::new(container.clone(), ty.clone()).display_source(db).ok()
            });
            let dir = port.direction.display_source(db).unwrap_or_default();
            param = match (dir.is_empty(), ty) {
                (false, Some(ty)) => format!("{dir} {ty} {port_name}"),
                (false, None) => format!("{dir} {port_name}"),
                (true, Some(ty)) => format!("{ty} {port_name}"),
                (true, None) => port_name.to_string(),
            };
        } else {
            param.push_str(port_name.as_str());
        }
        res.push_param(&param);

        match &active_param {
            Some(Either::Left(active_idx)) if *active_idx == idx => {
                res.active_parameter = Some(res.param_ranges.len() - 1);
            }
            Some(Either::Right(active_name)) if active_name == port_name.as_str() => {
                res.active_parameter = Some(res.param_ranges.len() - 1);
            }
            _ => {}
        }
    }

    res.label.push(')');
    Some(res)
}

/// The argument covering `offset`: its ordered index, or the connected port
/// name for a named argument. Mirrors the instance-connection logic above.
fn active_argument_at_offset(
    args: ast::ArgumentList<'_>,
    offset: TextSize,
) -> Option<Either<usize, String>> {
    for (idx, arg) in args.parameters().children().enumerate() {
        let range = arg.syntax().text_range()?;
        if range.end() >= offset {
            return match arg {
                ast::Argument::NamedArgument(named) => named
                    .name()
                    .and_then(|name| lower_ident_opt(Some(name)))
                    .map(|name| Either::Right(name.to_string())),
                _ => Some(Either::Left(idx)),
            };
        }
    }
    None
}

/// The `$name` of a system subroutine call, when `expr` is a system name.
fn system_identifier_of(expr: ast::Expression<'_>) -> Option<String> {
    match expr {
        ast::Expression::Name(ast::Name::SystemName(system)) => {
            system.system_identifier().map(|tok| tok.raw_text().to_string())
        }
        _ => None,
    }
}

fn sig_help_for_system_call(
    name: &str,
    invocation: ast::InvocationExpression<'_>,
    offset: TextSize,
    config: SignatureHelpConfig,
) -> Option<SignatureHelp> {
    let params = system_signature(name)?;

    let mut res = SignatureHelp::new(config, format!("{name}("));
    for param in params {
        res.push_param(param);
    }
    res.label.push(')');

    // System subroutines take positional arguments only; clamp the active
    // index to the last (variadic) slot so `...` highlights once the fixed
    // arguments are exhausted.
    if let Some(Some(Either::Left(idx))) =
        invocation.arguments().map(|args| active_argument_at_offset(args, offset))
        && let Some(last) = res.param_ranges.len().checked_sub(1)
    {
        res.active_parameter = Some(idx.min(last));
    }

    Some(res)
}

/// Parameter labels for a system subroutine, from the bundled
/// `system_signatures.toml` table. `...` marks variadic arguments.
fn system_signature(name: &str) -> Option<&'static [String]> {
    use std::sync::OnceLock;

    static TABLE: OnceLock<BTreeMap<String, SystemSignatureDef>> = OnceLock::new();
    TABLE
        .get_or_init(|| {
            toml::from_str(include_str!("system_signatures.toml"))
                .expect("bundled system_signatures.toml must parse")
        })
        .get(name)
        .map(|def| def.params.as_slice())
}

#[derive(Debug, serde::Deserialize)]
struct SystemSignatureDef {
    /// Read only by the slang-alignment test; the runtime path needs `params`.
    #[allow(dead_code)]
    kind: String,
    params: Vec<String>,
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashMap};

    use syntax::Compilation as SlangCompilation;

    use super::*;

    fn table() -> BTreeMap<String, SystemSignatureDef> {
        toml::from_str(include_str!("system_signatures.toml"))
            .expect("bundled system_signatures.toml must parse")
    }

    #[test]
    fn system_signature_table_matches_slang_subroutine_tables() {
        let table = table();
        let tasks: HashMap<String, bool> = SlangCompilation::system_task_names()
            .into_iter()
            .map(|name| (name, true))
            .chain(SlangCompilation::system_function_names().into_iter().map(|name| (name, false)))
            .collect();

        let mut unknown = Vec::new();
        let mut kind_mismatch = Vec::new();
        for (name, def) in &table {
            let Some(is_task) = tasks.get(name) else {
                unknown.push(name.clone());
                continue;
            };
            let expected = if *is_task { "task" } else { "function" };
            if def.kind != expected {
                kind_mismatch
                    .push(format!("{name}: table says {} but slang says {expected}", def.kind));
            }
        }

        assert!(unknown.is_empty(), "unknown system subroutines: {unknown:?}");
        assert!(kind_mismatch.is_empty(), "kind mismatches vs slang: {kind_mismatch:?}");
    }

    #[test]
    fn system_signature_table_has_unique_names() {
        let table = table();
        let names: BTreeSet<String> = table.keys().cloned().collect();
        assert_eq!(names.len(), table.len(), "duplicate names in system_signatures.toml");
    }
}
