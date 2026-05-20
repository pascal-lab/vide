use hir::{
    container::InContainer,
    hir_def::{
        declaration::Declaration,
        module::{Module, ModuleId, port::Ports},
    },
    semantics::Semantics,
};
use ide_db::root_db::RootDb;
use smol_str::SmolStr;
use utils::get::GetRef;

pub(crate) fn port_names(module: &Module) -> Vec<SmolStr> {
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

pub(crate) fn remaining_ordered_port_names(module: &Module, connected: usize) -> Vec<SmolStr> {
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

pub(crate) fn leading_parameter_names(module: &Module) -> Vec<SmolStr> {
    module
        .declarations
        .values()
        .take_while(|declaration| matches!(declaration, Declaration::ParamDecl(_)))
        .flat_map(|declaration| declaration.decls())
        .filter_map(|decl| module.get(decl).name.clone())
        .collect()
}

pub(crate) fn all_parameter_names(module: &Module) -> Vec<SmolStr> {
    module
        .declarations
        .values()
        .filter(|declaration| matches!(declaration, Declaration::ParamDecl(_)))
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
        (None, true) => format!("/* {name} */ {unresolved_ordered_value}"),
        (None, false) => format!(".{name}()"),
        (Some(_), true) => name.to_string(),
        (Some(_), false) => format!(".{name}"),
    }
}
