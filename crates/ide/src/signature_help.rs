use hir::{
    container::{InContainer, InModule},
    db::HirDb,
    display::HirDisplay,
    file::HirFileId,
    hir_def::{
        declaration::Declaration,
        module::{
            instantiation::{ParamAssign, PortConn},
            port::Ports,
        },
    },
    semantics::Semantics,
};
use ide_db::root_db::RootDb;
use itertools::Either;
use span::FilePosition;
use syntax::{
    SyntaxAncestors, SyntaxNodeExt,
    ast::{self, AstNode},
    has_text_range::HasTextRangeIn,
    match_ast,
};
// Last week, I found an issue with the original strategy and have successfully implemented
// most of the intrinsic in LSV. and find some optimization opportunities. This week's goal is
// to pass lit tests in IV and migrate some optimizations.
use utils::{
    get::{Get, GetRef},
    text_edit::{TextRange, TextSize},
};

use crate::markup::Markup;

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
    let root = sema.parse_root(file_id)?;
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
        let (module, module_src_map) = db.module_with_source_map(module_id);
        let instance = module.get(instance_id);

        let Some((idx, conn_id)) = instance.connections.iter().enumerate().find(|(_, conn_id)| {
            module_src_map.get(**conn_id).is_some_and(|src| src.node.range().end() >= offset)
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
    let target_module_id = sema.nameres_instantiation(instantiation)?;
    let target_module = db.module(target_module_id);
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
                                Ok(s) => buf.push_str(&s),
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
                    let header = InModule::new(target_module_id, port_decl.header)
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
                        && let Some(decl_name) = target_module.get(decl_id).name.as_ref()
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
        let (module, module_src_map) = db.module_with_source_map(module_id);
        let instantiation = module.get(instantiation_id);

        let Some((idx, conn_id)) =
            instantiation.param_assigns.iter().enumerate().find(|(_, conn_id)| {
                module_src_map.get(**conn_id).is_some_and(|src| src.node.range().end() >= offset)
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

    let target_module_id = sema.nameres_instantiation(instantiation)?;
    let target_module = db.module(target_module_id);
    let target_module_name =
        target_module.name.as_ref().map(|name| name.to_string()).unwrap_or("<module>".to_string());

    let mut res = SignatureHelp::new(config, format!("module {target_module_name} #("));

    if let Some(active_param) = &active_param {
        match active_param {
            Either::Left(idx) => res.active_parameter = Some(*idx),
            Either::Right(_) => {}
        }
    }

    for port_decl in
        target_module.declarations.values().take_while(|d| matches!(d, Declaration::ParamDecl(_)))
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
                && let Some(decl_name) = target_module.get(decl_id).name.as_ref()
                && active_name == decl_name.as_str()
            {
                res.active_parameter = Some(res.param_ranges.len() - 1);
            }
        }
    }

    res.label.push(')');
    Some(res)
}
