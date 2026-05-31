use super::{CodeActionCollector, CodeActionCtx};

pub(crate) type Handler = fn(&mut CodeActionCollector, &CodeActionCtx<'_>) -> Option<()>;

mod add_default_case_item;
mod add_implicit_named_port_parens;
mod add_instance_parens;
mod add_missing_connections;
mod add_missing_parameters;
mod apply_de_morgan;
mod convert_literal_base;
mod convert_ordered_connections;
mod expand_compound_assignment;
mod expand_postfix_inc_dec;
mod invert_if_else;
mod remove_empty_port_connections;
mod sort_named_instantiation_items;
mod split_declaration_declarators;
mod wrap_statement_in_begin_end;

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
        split_declaration_declarators::split_declaration_declarators,
        sort_named_instantiation_items::sort_named_parameter_assignments,
        sort_named_instantiation_items::sort_named_port_connections,
        add_default_case_item::add_default_case_item,
        invert_if_else::invert_if_else,
        wrap_statement_in_begin_end::unwrap_single_statement_block,
        wrap_statement_in_begin_end::wrap_statement_in_begin_end,
        expand_postfix_inc_dec::expand_postfix_inc_dec,
        expand_compound_assignment::expand_compound_assignment,
        apply_de_morgan::apply_de_morgan,
    ]
}
