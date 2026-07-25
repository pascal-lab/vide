use hir_def::{
    container::InContainer,
    declaration::Declaration,
    module::{Module, ModuleId, port::Ports},
    source_map::Lowered,
    symbol::Resolution,
};
use hir_semantics::semantics::Semantics;
use smol_str::SmolStr;

use crate::db::root_db::RootDb;

pub(crate) fn port_names(module: &Lowered<Module>) -> Vec<SmolStr> {
    match &module.ports {
        Ports::NonAnsi { ports, .. } => {
            ports.values().filter_map(|port| port.label.clone()).collect()
        }
        Ports::Ansi(ports) => ports
            .values()
            .flat_map(|port| port.decls.clone())
            .filter_map(|decl| module.get(decl).name.clone())
            .collect(),
    }
}

pub(crate) fn remaining_ordered_port_names(
    module: &Lowered<Module>,
    connected: usize,
) -> Vec<SmolStr> {
    match &module.ports {
        Ports::NonAnsi { ports, .. } => {
            ports.values().skip(connected).filter_map(|port| port.label.clone()).collect()
        }
        Ports::Ansi(ports) => ports
            .values()
            .flat_map(|port| port.decls.clone())
            .skip(connected)
            .filter_map(|decl| module.get(decl).name.clone())
            .collect(),
    }
}

pub(crate) fn leading_overridable_parameter_names(module: &Lowered<Module>) -> Vec<SmolStr> {
    module
        .declarations
        .values()
        .take_while(|declaration| matches!(declaration, Declaration::ParamDecl(_)))
        .filter(|declaration| {
            matches!(declaration, Declaration::ParamDecl(param_decl) if param_decl.kind.is_overridable())
        })
        .flat_map(|declaration| declaration.decls())
        .filter_map(|decl| module.get(decl).name.clone())
        .collect()
}

pub(crate) fn all_overridable_parameter_names(module: &Lowered<Module>) -> Vec<SmolStr> {
    module
        .declarations
        .values()
        .filter(|declaration| {
            matches!(declaration, Declaration::ParamDecl(param_decl) if param_decl.kind.is_overridable())
        })
        .flat_map(|declaration| declaration.decls())
        .filter_map(|decl| module.get(decl).name.clone())
        .collect()
}

pub(crate) fn missing_member_entry_text(
    sema: &Semantics<'_, RootDb>,
    module_id: ModuleId,
    name: SmolStr,
    is_ordered: bool,
    unresolved_ordered_value: &str,
) -> String {
    match (sema.name_to_def(InContainer::new(module_id.into(), name.clone())), is_ordered) {
        (Resolution::Unresolved, true) => {
            format!("/* {name} */ {unresolved_ordered_value}")
        }
        (Resolution::Unresolved, false) => format!(".{name}()"),
        (_, true) => name.to_string(),
        (_, false) => format!(".{name}"),
    }
}
