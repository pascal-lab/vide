; Keywords
[
  "begin"
  "end"
  "generate"
  "endgenerate"
  (module_keyword)
  "endmodule"
  "program"
  "endprogram"
  "package"
  "endpackage"
  "checker"
  "endchecker"
  "config"
  "endconfig"
  "pure"
  "virtual"
  "extends"
  "implements"
  "super"
  (class_item_qualifier)
  "parameter"
  "localparam"
  "defparam"
  "assign"
  "modport"
  "fork"
  "join"
  "join_none"
  "join_any"
  "forkjoin"
  "default"
  "break"
  "tagged"
  "extern"
  "alias"
  "posedge"
  "negedge"
  "bind"
  "expect"
  "type"
  "void"
  "coverpoint"
  "cross"
  "nettype"
  "export"
  "force"
  "release"
  "timeunit"
  "timeprecision"
  "sequence"
  "endsequence"
  "property"
  "endproperty"
  "clocking"
  "endclocking"
  "covergroup"
  "endgroup"
  "specify"
  "endspecify"
  "primitive"
  "endprimitive"
  "wait"
  "wait_order"
  "const"
  "constraint"
  "unique"
  "unique0"
  "do"
  "genvar"
  "inside"
  "continue"
  "randc"
  "event"
  "global"
  "ref"
  "initial"
  "final"
  "solve"
  "typedef"
  "class"
  "endclass"
  "interface"
  "endinterface"
  "enum"
  "struct"
  "union"
  (unique_priority)
  (bins_keyword)
  (always_keyword)
] @keyword

[
  "function"
  "endfunction"
  "task"
  "endtask"
] @keyword.function

"return" @keyword.return

[
  "for"
  "foreach"
  "repeat"
  "forever"
  "while"
] @keyword.repeat

[
  "if"
  "else"
  "iff"
  (case_keyword)
  "endcase"
] @keyword.conditional

[
  "="
  "-"
  "+"
  "/"
  "*"
  "^"
  "&"
  "|"
  "&&"
  "||"
  "<="
  "=="
  "!="
  "==="
  "!=="
  "-:"
  "<"
  ">"
  ">="
  "%"
  ">>"
  "<<"
  "|="
  "|=>"
  "|->"
  ">>>"
  "<<<"
  "->>"
  "->"
  "=>"
  "*>"
  ".*"
  (unary_operator)
  (inc_or_dec_operator)
  (queue_dimension)
] @operator

"#" @constructor

[
  ";"
  "::"
  ","
  "."
  ":"
] @punctuation.delimiter

(conditional_expression
  [
    "?"
    ":"
  ] @keyword.conditional.ternary)

[
  "["
  "]"
  "("
  ")"
  "{"
  "}"
  "'{"
] @punctuation.bracket

[
  "or"
  "and"
  "not"
  "nor"
  "nand"
  "xor"
  "xnor"
] @keyword.operator

[
  "input"
  "output"
  "inout"
  "var"
  "wire"
  "reg"
  "packed"
  "signed"
  "unsigned"
  "assert"
  "cover"
  "assume"
  "disable"
  "automatic"
  "static"
  "rand"
  (dpi_function_import_property)
  (dpi_task_import_property)
] @keyword.modifier

[
  "include"
  "import"
] @keyword.import

[
  (one_line_comment)
  (block_comment)
] @comment

[
  "@"
  (cycle_delay_range)
  (delay_control)
  (cycle_delay)
  (attribute_instance)
] @attribute

(attribute_instance
  (attr_spec
    (simple_identifier) @property))

[
  (integral_number)
  (unbased_unsized_literal)
  (fixed_point_number)
  (unsigned_number)
] @number

[
  (net_type)
  (data_type)
  (time_unit)
] @type.builtin

(list_of_variable_decl_assignments
  (variable_decl_assignment
    name: (simple_identifier) @variable))

(hierarchical_identifier
  (simple_identifier) @variable)

(tf_port_item
  (simple_identifier) @variable)

port_name: (simple_identifier) @variable

(port
  (simple_identifier) @variable)

(list_of_port_identifiers
  (simple_identifier) @variable)

(net_lvalue
  (simple_identifier) @variable)

(sequence_port_item
  (simple_identifier) @variable)

(property_port_item
  (simple_identifier) @variable)

(net_decl_assignment
  (simple_identifier) @variable)

(hierarchical_identifier
  (simple_identifier)
  (simple_identifier) @variable.member)

(select
  (simple_identifier) @variable.member)

(named_port_connection
  port_name: (simple_identifier) @variable.member)

(named_parameter_assignment
  (simple_identifier) @variable.parameter)

(parameter_declaration
  (list_of_param_assignments
    (param_assignment
      (simple_identifier) @variable.parameter)))

(local_parameter_declaration
  (list_of_param_assignments
    (param_assignment
      (simple_identifier) @variable.parameter)))

[
  (simulation_control_task)
  (system_tf_identifier)
  (severity_system_task)
  (randomize_call)
  (array_or_queue_method_name)
  "new"
] @function.builtin

(task_body_declaration
  .
  name: (simple_identifier) @function
  (simple_identifier)? @label)

(function_body_declaration
  .
  name: (simple_identifier) @function
  (simple_identifier)? @label)

(function_body_declaration
  .
  (data_type_or_void)
  name: (simple_identifier) @function
  (simple_identifier)? @label)

(clocking_declaration
  .
  name: (simple_identifier) @constructor
  (simple_identifier)? @label)

(sequence_declaration
  .
  name: (simple_identifier) @constructor
  (simple_identifier)? @label)

(property_declaration
  .
  name: (simple_identifier) @constructor
  (simple_identifier)? @label)

(class_declaration
  .
  name: (simple_identifier) @constructor
  (simple_identifier)? @label)

(interface_class_declaration
  .
  name: (simple_identifier) @constructor
  (simple_identifier)? @label)

(covergroup_declaration
  .
  name: (simple_identifier) @constructor
  (simple_identifier)? @label)

(package_declaration
  .
  name: (simple_identifier) @constructor
  (simple_identifier)? @label)

(checker_declaration
  .
  name: (simple_identifier) @constructor
  (simple_identifier)? @label)

(interface_declaration
  .
  [
    (simple_identifier) @constructor
    (interface_nonansi_header
      (simple_identifier) @constructor)
    (interface_ansi_header
      (simple_identifier) @constructor)
  ]
  (simple_identifier)? @label)

(module_declaration
  .
  [
    (simple_identifier) @constructor
    (module_nonansi_header
      (simple_identifier) @constructor)
    (module_ansi_header
      (simple_identifier) @constructor)
  ]
  (simple_identifier)? @label)

(program_declaration
  .
  [
    (simple_identifier) @constructor
    (program_nonansi_header
      (simple_identifier) @constructor)
    (program_ansi_header
      (simple_identifier) @constructor)
  ]
  (simple_identifier)? @label)

(method_call_body
  name: (simple_identifier) @function.call)

(tf_call
  (hierarchical_identifier
    (simple_identifier) @function.call))

(module_instantiation
  instance_type: (simple_identifier) @constructor)

(name_of_instance
  instance_name: (simple_identifier) @module)

(sequence_instance
  (hierarchical_identifier
    (simple_identifier) @module))

(udp_instantiation
  (simple_identifier) @constructor)

(bind_directive
  (bind_target_scope
    (simple_identifier) @constructor))

(bind_target_instance
  (hierarchical_identifier
    (simple_identifier) @module))

(concurrent_assertion_item
  (simple_identifier) @label)

(cover_point
  name: (simple_identifier) @label)

(cover_cross
  name: (simple_identifier) @module)

(list_of_cross_items
  (simple_identifier) @constructor)

(package_import_item
  (simple_identifier) @constructor)

(seq_block
  (simple_identifier) @label)

(statement
  block_name: (simple_identifier) @label)

(dpi_spec_string) @string

c_name: (c_identifier) @function

(dpi_import_export
  name: (simple_identifier) @function)

(class_type
  (simple_identifier) @constructor)

(class_type
  (simple_identifier)
  (simple_identifier) @type)

(task_prototype
  name: (simple_identifier) @function)

(function_prototype
  name: (simple_identifier) @function)

(type_assignment
  name: (simple_identifier) @type.definition)

(interface_class_type
  (simple_identifier) @type.definition)

(package_scope
  (simple_identifier) @constructor)

(data_declaration
  (type_declaration
    type_name: (simple_identifier) @type.definition))

(net_declaration
  (simple_identifier) @type)

(constraint_declaration
  (simple_identifier) @constructor)

(method_call
  (primary
    (hierarchical_identifier
      (simple_identifier) @constructor)))

(string_literal
  (quoted_string) @string)

(include_statement
  (file_path_spec) @string)

[
  "`define"
  "`default_nettype"
  (resetall_compiler_directive)
  (include_compiler_directive)
  "`timescale"
  "`undef"
  (undefineall_compiler_directive)
  "`ifdef"
  "`ifndef"
  "`elsif"
  "`endif"
  "`else"
] @keyword.directive.define

(include_compiler_directive
  (quoted_string) @string)

(include_compiler_directive
  (system_lib_string) @string)

(default_nettype_compiler_directive
  (default_nettype_value) @type.builtin)

(text_macro_definition
  (text_macro_name
    (simple_identifier) @keyword.directive))

(text_macro_usage) @keyword.directive

(ifdef_condition
  (simple_identifier) @keyword.directive)

(undefine_compiler_directive
  (simple_identifier) @keyword.directive)
