use super::{CodeActionCollector, CodeActionCtx};

pub(crate) type Handler = fn(&mut CodeActionCollector, &CodeActionCtx<'_>) -> Option<()>;

mod add_implicit_named_port_parens;
mod add_instance_parens;
mod add_missing_connections;
mod add_missing_parameters;
mod convert_literal_base;
mod convert_ordered_connections;
mod remove_empty_port_connections;

pub(crate) fn all() -> &'static [Handler] {
    &[
        convert_literal_base::convert_literal_base,
        add_missing_connections::add_missing_connections,
        add_missing_parameters::add_missing_parameters,
        convert_ordered_connections::convert_ordered_ports,
        convert_ordered_connections::convert_ordered_params,
        remove_empty_port_connections::remove_empty_port_connections,
        add_implicit_named_port_parens::add_implicit_named_port_parens,
        add_instance_parens::add_instance_parens,
    ]
}
