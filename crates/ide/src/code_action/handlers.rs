use super::{CodeActionCollector, CodeActionCtx};

pub(crate) type Handler = fn(&mut CodeActionCollector, &CodeActionCtx<'_>) -> Option<()>;

mod add_default_case_item;
mod add_implicit_named_port_parens;
mod add_instance_parens;
mod add_missing_connections;
mod add_missing_parameters;
mod apply_de_morgan;
mod convert_always_block;
mod convert_literal_base;
mod convert_named_port_connections;
mod convert_ordered_connections;
mod convert_port_declarations;
mod expand_compound_assignment;
mod expand_postfix_inc_dec;
mod extract_variable;
mod insert_expected_token;
mod invert_if_else;
mod merge_nested_if;
mod pull_assignment_up;
mod reformat_number_literal;
mod remove_empty_port_connections;
mod remove_parentheses;
mod sort_named_instantiation_items;
mod split_declaration_declarators;
mod wrap_statement_in_begin_end;

pub(crate) fn all() -> &'static [Handler] {
    &[
        convert_literal_base::convert_literal_base,
        reformat_number_literal::reformat_number_literal,
        add_missing_connections::add_missing_connections,
        add_missing_parameters::add_missing_parameters,
        convert_ordered_connections::convert_ordered_ports,
        convert_ordered_connections::convert_ordered_params,
        convert_named_port_connections::convert_named_port_connection_shorthand,
        remove_empty_port_connections::remove_empty_port_connections,
        add_implicit_named_port_parens::add_implicit_named_port_parens,
        add_instance_parens::add_instance_parens,
        convert_always_block::convert_always_block,
        convert_port_declarations::convert_port_declarations,
        split_declaration_declarators::split_declaration_declarators,
        sort_named_instantiation_items::sort_named_parameter_assignments,
        sort_named_instantiation_items::sort_named_port_connections,
        add_default_case_item::add_default_case_item,
        invert_if_else::invert_if_else,
        merge_nested_if::merge_nested_if,
        wrap_statement_in_begin_end::unwrap_single_statement_block,
        wrap_statement_in_begin_end::wrap_statement_in_begin_end,
        remove_parentheses::remove_parentheses,
        expand_postfix_inc_dec::expand_postfix_inc_dec,
        expand_compound_assignment::expand_compound_assignment,
        extract_variable::extract_variable,
        pull_assignment_up::pull_assignment_up,
        pull_assignment_up::pull_assignment_down,
        apply_de_morgan::apply_de_morgan,
        insert_expected_token::insert_expected_token,
    ]
}
