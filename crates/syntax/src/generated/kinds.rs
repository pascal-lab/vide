use std::fmt;
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SyntaxKind(u16);
impl SyntaxKind {
    pub const ACCEPT_ON_PROPERTY_EXPR: Self = Self(4u16);
    pub const ACTION_BLOCK: Self = Self(5u16);
    pub const ADD_ASSIGNMENT_EXPRESSION: Self = Self(6u16);
    pub const ADD_EXPRESSION: Self = Self(7u16);
    pub const ALL: &'static [Self] = &[
        Self::UNKNOWN,
        Self::SYNTAX_LIST,
        Self::TOKEN_LIST,
        Self::SEPARATED_LIST,
        Self::ACCEPT_ON_PROPERTY_EXPR,
        Self::ACTION_BLOCK,
        Self::ADD_ASSIGNMENT_EXPRESSION,
        Self::ADD_EXPRESSION,
        Self::ALWAYS_BLOCK,
        Self::ALWAYS_COMB_BLOCK,
        Self::ALWAYS_FF_BLOCK,
        Self::ALWAYS_LATCH_BLOCK,
        Self::AND_ASSIGNMENT_EXPRESSION,
        Self::AND_PROPERTY_EXPR,
        Self::AND_SEQUENCE_EXPR,
        Self::ANONYMOUS_PROGRAM,
        Self::ANSI_PORT_LIST,
        Self::ANSI_UDP_PORT_LIST,
        Self::ARGUMENT_LIST,
        Self::ARITHMETIC_LEFT_SHIFT_ASSIGNMENT_EXPRESSION,
        Self::ARITHMETIC_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION,
        Self::ARITHMETIC_SHIFT_LEFT_EXPRESSION,
        Self::ARITHMETIC_SHIFT_RIGHT_EXPRESSION,
        Self::ARRAY_AND_METHOD,
        Self::ARRAY_OR_METHOD,
        Self::ARRAY_OR_RANDOMIZE_METHOD_EXPRESSION,
        Self::ARRAY_UNIQUE_METHOD,
        Self::ARRAY_XOR_METHOD,
        Self::ASCENDING_RANGE_SELECT,
        Self::ASSERT_PROPERTY_STATEMENT,
        Self::ASSERTION_ITEM_PORT,
        Self::ASSERTION_ITEM_PORT_LIST,
        Self::ASSIGNMENT_EXPRESSION,
        Self::ASSIGNMENT_PATTERN_EXPRESSION,
        Self::ASSIGNMENT_PATTERN_ITEM,
        Self::ASSUME_PROPERTY_STATEMENT,
        Self::ATTRIBUTE_INSTANCE,
        Self::ATTRIBUTE_SPEC,
        Self::BAD_EXPRESSION,
        Self::BEGIN_KEYWORDS_DIRECTIVE,
        Self::BIN_SELECT_WITH_FILTER_EXPR,
        Self::BINARY_AND_EXPRESSION,
        Self::BINARY_BINS_SELECT_EXPR,
        Self::BINARY_BLOCK_EVENT_EXPRESSION,
        Self::BINARY_CONDITIONAL_DIRECTIVE_EXPRESSION,
        Self::BINARY_EVENT_EXPRESSION,
        Self::BINARY_OR_EXPRESSION,
        Self::BINARY_XNOR_EXPRESSION,
        Self::BINARY_XOR_EXPRESSION,
        Self::BIND_DIRECTIVE,
        Self::BIND_TARGET_LIST,
        Self::BINS_SELECT_CONDITION_EXPR,
        Self::BINS_SELECTION,
        Self::BIT_SELECT,
        Self::BIT_TYPE,
        Self::BLOCK_COVERAGE_EVENT,
        Self::BLOCKING_EVENT_TRIGGER_STATEMENT,
        Self::BYTE_TYPE,
        Self::C_HANDLE_TYPE,
        Self::CASE_EQUALITY_EXPRESSION,
        Self::CASE_GENERATE,
        Self::CASE_INEQUALITY_EXPRESSION,
        Self::CASE_PROPERTY_EXPR,
        Self::CASE_STATEMENT,
        Self::CAST_EXPRESSION,
        Self::CELL_CONFIG_RULE,
        Self::CELL_DEFINE_DIRECTIVE,
        Self::CHARGE_STRENGTH,
        Self::CHECKER_DATA_DECLARATION,
        Self::CHECKER_DECLARATION,
        Self::CHECKER_INSTANCE_STATEMENT,
        Self::CHECKER_INSTANTIATION,
        Self::CLASS_DECLARATION,
        Self::CLASS_METHOD_DECLARATION,
        Self::CLASS_METHOD_PROTOTYPE,
        Self::CLASS_NAME,
        Self::CLASS_PROPERTY_DECLARATION,
        Self::CLASS_SPECIFIER,
        Self::CLOCKING_DECLARATION,
        Self::CLOCKING_DIRECTION,
        Self::CLOCKING_ITEM,
        Self::CLOCKING_PROPERTY_EXPR,
        Self::CLOCKING_SEQUENCE_EXPR,
        Self::CLOCKING_SKEW,
        Self::COLON_EXPRESSION_CLAUSE,
        Self::COMPILATION_UNIT,
        Self::CONCATENATION_EXPRESSION,
        Self::CONCURRENT_ASSERTION_MEMBER,
        Self::CONDITIONAL_CONSTRAINT,
        Self::CONDITIONAL_EXPRESSION,
        Self::CONDITIONAL_PATH_DECLARATION,
        Self::CONDITIONAL_PATTERN,
        Self::CONDITIONAL_PREDICATE,
        Self::CONDITIONAL_PROPERTY_EXPR,
        Self::CONDITIONAL_STATEMENT,
        Self::CONFIG_CELL_IDENTIFIER,
        Self::CONFIG_DECLARATION,
        Self::CONFIG_INSTANCE_IDENTIFIER,
        Self::CONFIG_LIBLIST,
        Self::CONFIG_USE_CLAUSE,
        Self::CONSTRAINT_BLOCK,
        Self::CONSTRAINT_DECLARATION,
        Self::CONSTRAINT_PROTOTYPE,
        Self::CONSTRUCTOR_NAME,
        Self::CONTINUOUS_ASSIGN,
        Self::COPY_CLASS_EXPRESSION,
        Self::COVER_CROSS,
        Self::COVER_PROPERTY_STATEMENT,
        Self::COVER_SEQUENCE_STATEMENT,
        Self::COVERAGE_BINS,
        Self::COVERAGE_BINS_ARRAY_SIZE,
        Self::COVERAGE_IFF_CLAUSE,
        Self::COVERAGE_OPTION,
        Self::COVERGROUP_DECLARATION,
        Self::COVERPOINT,
        Self::CYCLE_DELAY,
        Self::DPI_EXPORT,
        Self::DPI_IMPORT,
        Self::DATA_DECLARATION,
        Self::DECLARATOR,
        Self::DEF_PARAM,
        Self::DEF_PARAM_ASSIGNMENT,
        Self::DEFAULT_CASE_ITEM,
        Self::DEFAULT_CLOCKING_REFERENCE,
        Self::DEFAULT_CONFIG_RULE,
        Self::DEFAULT_COVERAGE_BIN_INITIALIZER,
        Self::DEFAULT_DECAY_TIME_DIRECTIVE,
        Self::DEFAULT_DISABLE_DECLARATION,
        Self::DEFAULT_DIST_ITEM,
        Self::DEFAULT_EXTENDS_CLAUSE_ARG,
        Self::DEFAULT_FUNCTION_PORT,
        Self::DEFAULT_NET_TYPE_DIRECTIVE,
        Self::DEFAULT_PATTERN_KEY_EXPRESSION,
        Self::DEFAULT_PROPERTY_CASE_ITEM,
        Self::DEFAULT_RS_CASE_ITEM,
        Self::DEFAULT_SKEW_ITEM,
        Self::DEFAULT_TRIREG_STRENGTH_DIRECTIVE,
        Self::DEFERRED_ASSERTION,
        Self::DEFINE_DIRECTIVE,
        Self::DELAY_3,
        Self::DELAY_CONTROL,
        Self::DELAY_MODE_DISTRIBUTED_DIRECTIVE,
        Self::DELAY_MODE_PATH_DIRECTIVE,
        Self::DELAY_MODE_UNIT_DIRECTIVE,
        Self::DELAY_MODE_ZERO_DIRECTIVE,
        Self::DELAYED_SEQUENCE_ELEMENT,
        Self::DELAYED_SEQUENCE_EXPR,
        Self::DESCENDING_RANGE_SELECT,
        Self::DISABLE_CONSTRAINT,
        Self::DISABLE_FORK_STATEMENT,
        Self::DISABLE_IFF,
        Self::DISABLE_STATEMENT,
        Self::DIST_CONSTRAINT_LIST,
        Self::DIST_ITEM,
        Self::DIST_WEIGHT,
        Self::DIVIDE_ASSIGNMENT_EXPRESSION,
        Self::DIVIDE_EXPRESSION,
        Self::DIVIDER_CLAUSE,
        Self::DO_WHILE_STATEMENT,
        Self::DOT_MEMBER_CLAUSE,
        Self::DRIVE_STRENGTH,
        Self::EDGE_CONTROL_SPECIFIER,
        Self::EDGE_DESCRIPTOR,
        Self::EDGE_SENSITIVE_PATH_SUFFIX,
        Self::ELAB_SYSTEM_TASK,
        Self::ELEMENT_SELECT,
        Self::ELEMENT_SELECT_EXPRESSION,
        Self::ELS_IF_DIRECTIVE,
        Self::ELSE_CLAUSE,
        Self::ELSE_CONSTRAINT_CLAUSE,
        Self::ELSE_DIRECTIVE,
        Self::ELSE_PROPERTY_CLAUSE,
        Self::EMPTY_ARGUMENT,
        Self::EMPTY_IDENTIFIER_NAME,
        Self::EMPTY_MEMBER,
        Self::EMPTY_NON_ANSI_PORT,
        Self::EMPTY_PORT_CONNECTION,
        Self::EMPTY_QUEUE_EXPRESSION,
        Self::EMPTY_STATEMENT,
        Self::EMPTY_TIMING_CHECK_ARG,
        Self::END_CELL_DEFINE_DIRECTIVE,
        Self::END_IF_DIRECTIVE,
        Self::END_KEYWORDS_DIRECTIVE,
        Self::END_PROTECT_DIRECTIVE,
        Self::END_PROTECTED_DIRECTIVE,
        Self::ENUM_TYPE,
        Self::EQUALITY_EXPRESSION,
        Self::EQUALS_ASSERTION_ARG_CLAUSE,
        Self::EQUALS_TYPE_CLAUSE,
        Self::EQUALS_VALUE_CLAUSE,
        Self::EVENT_CONTROL,
        Self::EVENT_CONTROL_WITH_EXPRESSION,
        Self::EVENT_TYPE,
        Self::EXPECT_PROPERTY_STATEMENT,
        Self::EXPLICIT_ANSI_PORT,
        Self::EXPLICIT_NON_ANSI_PORT,
        Self::EXPRESSION_CONSTRAINT,
        Self::EXPRESSION_COVERAGE_BIN_INITIALIZER,
        Self::EXPRESSION_OR_DIST,
        Self::EXPRESSION_PATTERN,
        Self::EXPRESSION_STATEMENT,
        Self::EXPRESSION_TIMING_CHECK_ARG,
        Self::EXTENDS_CLAUSE,
        Self::EXTERN_INTERFACE_METHOD,
        Self::EXTERN_MODULE_DECL,
        Self::EXTERN_UDP_DECL,
        Self::FILE_PATH_SPEC,
        Self::FINAL_BLOCK,
        Self::FIRST_MATCH_SEQUENCE_EXPR,
        Self::FOLLOWED_BY_PROPERTY_EXPR,
        Self::FOR_LOOP_STATEMENT,
        Self::FOR_VARIABLE_DECLARATION,
        Self::FOREACH_LOOP_LIST,
        Self::FOREACH_LOOP_STATEMENT,
        Self::FOREVER_STATEMENT,
        Self::FORWARD_TYPE_RESTRICTION,
        Self::FORWARD_TYPEDEF_DECLARATION,
        Self::FUNCTION_DECLARATION,
        Self::FUNCTION_PORT,
        Self::FUNCTION_PORT_LIST,
        Self::FUNCTION_PROTOTYPE,
        Self::GENERATE_BLOCK,
        Self::GENERATE_REGION,
        Self::GENVAR_DECLARATION,
        Self::GREATER_THAN_EQUAL_EXPRESSION,
        Self::GREATER_THAN_EXPRESSION,
        Self::HIERARCHICAL_INSTANCE,
        Self::HIERARCHY_INSTANTIATION,
        Self::ID_WITH_EXPR_COVERAGE_BIN_INITIALIZER,
        Self::IDENTIFIER_NAME,
        Self::IDENTIFIER_SELECT_NAME,
        Self::IF_DEF_DIRECTIVE,
        Self::IF_GENERATE,
        Self::IF_N_DEF_DIRECTIVE,
        Self::IF_NONE_PATH_DECLARATION,
        Self::IFF_EVENT_CLAUSE,
        Self::IFF_PROPERTY_EXPR,
        Self::IMMEDIATE_ASSERT_STATEMENT,
        Self::IMMEDIATE_ASSERTION_MEMBER,
        Self::IMMEDIATE_ASSUME_STATEMENT,
        Self::IMMEDIATE_COVER_STATEMENT,
        Self::IMPLEMENTS_CLAUSE,
        Self::IMPLICATION_CONSTRAINT,
        Self::IMPLICATION_PROPERTY_EXPR,
        Self::IMPLICIT_ANSI_PORT,
        Self::IMPLICIT_EVENT_CONTROL,
        Self::IMPLICIT_NON_ANSI_PORT,
        Self::IMPLICIT_TYPE,
        Self::IMPLIES_PROPERTY_EXPR,
        Self::INCLUDE_DIRECTIVE,
        Self::INEQUALITY_EXPRESSION,
        Self::INITIAL_BLOCK,
        Self::INSIDE_EXPRESSION,
        Self::INSTANCE_CONFIG_RULE,
        Self::INSTANCE_NAME,
        Self::INT_TYPE,
        Self::INTEGER_LITERAL_EXPRESSION,
        Self::INTEGER_TYPE,
        Self::INTEGER_VECTOR_EXPRESSION,
        Self::INTERFACE_DECLARATION,
        Self::INTERFACE_HEADER,
        Self::INTERFACE_PORT_HEADER,
        Self::INTERSECT_CLAUSE,
        Self::INTERSECT_SEQUENCE_EXPR,
        Self::INVOCATION_EXPRESSION,
        Self::JUMP_STATEMENT,
        Self::LESS_THAN_EQUAL_EXPRESSION,
        Self::LESS_THAN_EXPRESSION,
        Self::LET_DECLARATION,
        Self::LIBRARY_DECLARATION,
        Self::LIBRARY_INC_DIR_CLAUSE,
        Self::LIBRARY_INCLUDE_STATEMENT,
        Self::LIBRARY_MAP,
        Self::LINE_DIRECTIVE,
        Self::LOCAL_SCOPE,
        Self::LOCAL_VARIABLE_DECLARATION,
        Self::LOGIC_TYPE,
        Self::LOGICAL_AND_EXPRESSION,
        Self::LOGICAL_EQUIVALENCE_EXPRESSION,
        Self::LOGICAL_IMPLICATION_EXPRESSION,
        Self::LOGICAL_LEFT_SHIFT_ASSIGNMENT_EXPRESSION,
        Self::LOGICAL_OR_EXPRESSION,
        Self::LOGICAL_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION,
        Self::LOGICAL_SHIFT_LEFT_EXPRESSION,
        Self::LOGICAL_SHIFT_RIGHT_EXPRESSION,
        Self::LONG_INT_TYPE,
        Self::LOOP_CONSTRAINT,
        Self::LOOP_GENERATE,
        Self::LOOP_STATEMENT,
        Self::MACRO_ACTUAL_ARGUMENT,
        Self::MACRO_ACTUAL_ARGUMENT_LIST,
        Self::MACRO_ARGUMENT_DEFAULT,
        Self::MACRO_FORMAL_ARGUMENT,
        Self::MACRO_FORMAL_ARGUMENT_LIST,
        Self::MACRO_USAGE,
        Self::MATCHES_CLAUSE,
        Self::MEMBER_ACCESS_EXPRESSION,
        Self::MIN_TYP_MAX_EXPRESSION,
        Self::MOD_ASSIGNMENT_EXPRESSION,
        Self::MOD_EXPRESSION,
        Self::MODPORT_CLOCKING_PORT,
        Self::MODPORT_DECLARATION,
        Self::MODPORT_EXPLICIT_PORT,
        Self::MODPORT_ITEM,
        Self::MODPORT_NAMED_PORT,
        Self::MODPORT_SIMPLE_PORT_LIST,
        Self::MODPORT_SUBROUTINE_PORT,
        Self::MODPORT_SUBROUTINE_PORT_LIST,
        Self::MODULE_DECLARATION,
        Self::MODULE_HEADER,
        Self::MULTIPLE_CONCATENATION_EXPRESSION,
        Self::MULTIPLY_ASSIGNMENT_EXPRESSION,
        Self::MULTIPLY_EXPRESSION,
        Self::NAME_VALUE_PRAGMA_EXPRESSION,
        Self::NAMED_ARGUMENT,
        Self::NAMED_BLOCK_CLAUSE,
        Self::NAMED_CONDITIONAL_DIRECTIVE_EXPRESSION,
        Self::NAMED_LABEL,
        Self::NAMED_PARAM_ASSIGNMENT,
        Self::NAMED_PORT_CONNECTION,
        Self::NAMED_STRUCTURE_PATTERN_MEMBER,
        Self::NAMED_TYPE,
        Self::NET_ALIAS,
        Self::NET_DECLARATION,
        Self::NET_PORT_HEADER,
        Self::NET_TYPE_DECLARATION,
        Self::NEW_ARRAY_EXPRESSION,
        Self::NEW_CLASS_EXPRESSION,
        Self::NO_UNCONNECTED_DRIVE_DIRECTIVE,
        Self::NON_ANSI_PORT_LIST,
        Self::NON_ANSI_UDP_PORT_LIST,
        Self::NONBLOCKING_ASSIGNMENT_EXPRESSION,
        Self::NONBLOCKING_EVENT_TRIGGER_STATEMENT,
        Self::NULL_LITERAL_EXPRESSION,
        Self::NUMBER_PRAGMA_EXPRESSION,
        Self::ONE_STEP_DELAY,
        Self::OR_ASSIGNMENT_EXPRESSION,
        Self::OR_PROPERTY_EXPR,
        Self::OR_SEQUENCE_EXPR,
        Self::ORDERED_ARGUMENT,
        Self::ORDERED_PARAM_ASSIGNMENT,
        Self::ORDERED_PORT_CONNECTION,
        Self::ORDERED_STRUCTURE_PATTERN_MEMBER,
        Self::PACKAGE_DECLARATION,
        Self::PACKAGE_EXPORT_ALL_DECLARATION,
        Self::PACKAGE_EXPORT_DECLARATION,
        Self::PACKAGE_HEADER,
        Self::PACKAGE_IMPORT_DECLARATION,
        Self::PACKAGE_IMPORT_ITEM,
        Self::PARALLEL_BLOCK_STATEMENT,
        Self::PARAMETER_DECLARATION,
        Self::PARAMETER_DECLARATION_STATEMENT,
        Self::PARAMETER_PORT_LIST,
        Self::PARAMETER_VALUE_ASSIGNMENT,
        Self::PAREN_EXPRESSION_LIST,
        Self::PAREN_PRAGMA_EXPRESSION,
        Self::PARENTHESIZED_BINS_SELECT_EXPR,
        Self::PARENTHESIZED_CONDITIONAL_DIRECTIVE_EXPRESSION,
        Self::PARENTHESIZED_EVENT_EXPRESSION,
        Self::PARENTHESIZED_EXPRESSION,
        Self::PARENTHESIZED_PATTERN,
        Self::PARENTHESIZED_PROPERTY_EXPR,
        Self::PARENTHESIZED_SEQUENCE_EXPR,
        Self::PATH_DECLARATION,
        Self::PATH_DESCRIPTION,
        Self::PATTERN_CASE_ITEM,
        Self::PORT_CONCATENATION,
        Self::PORT_DECLARATION,
        Self::PORT_REFERENCE,
        Self::POSTDECREMENT_EXPRESSION,
        Self::POSTINCREMENT_EXPRESSION,
        Self::POWER_EXPRESSION,
        Self::PRAGMA_DIRECTIVE,
        Self::PRIMARY_BLOCK_EVENT_EXPRESSION,
        Self::PRIMITIVE_INSTANTIATION,
        Self::PROCEDURAL_ASSIGN_STATEMENT,
        Self::PROCEDURAL_DEASSIGN_STATEMENT,
        Self::PROCEDURAL_FORCE_STATEMENT,
        Self::PROCEDURAL_RELEASE_STATEMENT,
        Self::PRODUCTION,
        Self::PROGRAM_DECLARATION,
        Self::PROGRAM_HEADER,
        Self::PROPERTY_DECLARATION,
        Self::PROPERTY_SPEC,
        Self::PROPERTY_TYPE,
        Self::PROTECT_DIRECTIVE,
        Self::PROTECTED_DIRECTIVE,
        Self::PULL_STRENGTH,
        Self::PULSE_STYLE_DECLARATION,
        Self::QUEUE_DIMENSION_SPECIFIER,
        Self::RAND_CASE_ITEM,
        Self::RAND_CASE_STATEMENT,
        Self::RAND_JOIN_CLAUSE,
        Self::RAND_SEQUENCE_STATEMENT,
        Self::RANGE_COVERAGE_BIN_INITIALIZER,
        Self::RANGE_DIMENSION_SPECIFIER,
        Self::RANGE_LIST,
        Self::REAL_LITERAL_EXPRESSION,
        Self::REAL_TIME_TYPE,
        Self::REAL_TYPE,
        Self::REG_TYPE,
        Self::REPEATED_EVENT_CONTROL,
        Self::REPLICATED_ASSIGNMENT_PATTERN,
        Self::RESET_ALL_DIRECTIVE,
        Self::RESTRICT_PROPERTY_STATEMENT,
        Self::RETURN_STATEMENT,
        Self::ROOT_SCOPE,
        Self::RS_CASE,
        Self::RS_CODE_BLOCK,
        Self::RS_ELSE_CLAUSE,
        Self::RS_IF_ELSE,
        Self::RS_PROD_ITEM,
        Self::RS_REPEAT,
        Self::RS_RULE,
        Self::RS_WEIGHT_CLAUSE,
        Self::S_UNTIL_PROPERTY_EXPR,
        Self::S_UNTIL_WITH_PROPERTY_EXPR,
        Self::SCOPED_NAME,
        Self::SEQUENCE_DECLARATION,
        Self::SEQUENCE_MATCH_LIST,
        Self::SEQUENCE_REPETITION,
        Self::SEQUENCE_TYPE,
        Self::SEQUENTIAL_BLOCK_STATEMENT,
        Self::SHORT_INT_TYPE,
        Self::SHORT_REAL_TYPE,
        Self::SIGNAL_EVENT_EXPRESSION,
        Self::SIGNED_CAST_EXPRESSION,
        Self::SIMPLE_ASSIGNMENT_PATTERN,
        Self::SIMPLE_BINS_SELECT_EXPR,
        Self::SIMPLE_PATH_SUFFIX,
        Self::SIMPLE_PRAGMA_EXPRESSION,
        Self::SIMPLE_PROPERTY_EXPR,
        Self::SIMPLE_RANGE_SELECT,
        Self::SIMPLE_SEQUENCE_EXPR,
        Self::SOLVE_BEFORE_CONSTRAINT,
        Self::SPECIFY_BLOCK,
        Self::SPECPARAM_DECLARATION,
        Self::SPECPARAM_DECLARATOR,
        Self::STANDARD_CASE_ITEM,
        Self::STANDARD_PROPERTY_CASE_ITEM,
        Self::STANDARD_RS_CASE_ITEM,
        Self::STREAM_EXPRESSION,
        Self::STREAM_EXPRESSION_WITH_RANGE,
        Self::STREAMING_CONCATENATION_EXPRESSION,
        Self::STRING_LITERAL_EXPRESSION,
        Self::STRING_TYPE,
        Self::STRONG_WEAK_PROPERTY_EXPR,
        Self::STRUCT_TYPE,
        Self::STRUCT_UNION_MEMBER,
        Self::STRUCTURE_PATTERN,
        Self::STRUCTURED_ASSIGNMENT_PATTERN,
        Self::SUBTRACT_ASSIGNMENT_EXPRESSION,
        Self::SUBTRACT_EXPRESSION,
        Self::SUPER_HANDLE,
        Self::SUPER_NEW_DEFAULTED_ARGS_EXPRESSION,
        Self::SYSTEM_NAME,
        Self::SYSTEM_TIMING_CHECK,
        Self::TAGGED_PATTERN,
        Self::TAGGED_UNION_EXPRESSION,
        Self::TASK_DECLARATION,
        Self::THIS_HANDLE,
        Self::THROUGHOUT_SEQUENCE_EXPR,
        Self::TIME_LITERAL_EXPRESSION,
        Self::TIME_SCALE_DIRECTIVE,
        Self::TIME_TYPE,
        Self::TIME_UNITS_DECLARATION,
        Self::TIMING_CHECK_EVENT_ARG,
        Self::TIMING_CHECK_EVENT_CONDITION,
        Self::TIMING_CONTROL_EXPRESSION,
        Self::TIMING_CONTROL_STATEMENT,
        Self::TRANS_LIST_COVERAGE_BIN_INITIALIZER,
        Self::TRANS_RANGE,
        Self::TRANS_REPEAT_RANGE,
        Self::TRANS_SET,
        Self::TYPE_ASSIGNMENT,
        Self::TYPE_PARAMETER_DECLARATION,
        Self::TYPE_REFERENCE,
        Self::TYPEDEF_DECLARATION,
        Self::UDP_BODY,
        Self::UDP_DECLARATION,
        Self::UDP_EDGE_FIELD,
        Self::UDP_ENTRY,
        Self::UDP_INITIAL_STMT,
        Self::UDP_INPUT_PORT_DECL,
        Self::UDP_OUTPUT_PORT_DECL,
        Self::UDP_SIMPLE_FIELD,
        Self::UNARY_BINS_SELECT_EXPR,
        Self::UNARY_BITWISE_AND_EXPRESSION,
        Self::UNARY_BITWISE_NAND_EXPRESSION,
        Self::UNARY_BITWISE_NOR_EXPRESSION,
        Self::UNARY_BITWISE_NOT_EXPRESSION,
        Self::UNARY_BITWISE_OR_EXPRESSION,
        Self::UNARY_BITWISE_XNOR_EXPRESSION,
        Self::UNARY_BITWISE_XOR_EXPRESSION,
        Self::UNARY_CONDITIONAL_DIRECTIVE_EXPRESSION,
        Self::UNARY_LOGICAL_NOT_EXPRESSION,
        Self::UNARY_MINUS_EXPRESSION,
        Self::UNARY_PLUS_EXPRESSION,
        Self::UNARY_PREDECREMENT_EXPRESSION,
        Self::UNARY_PREINCREMENT_EXPRESSION,
        Self::UNARY_PROPERTY_EXPR,
        Self::UNARY_SELECT_PROPERTY_EXPR,
        Self::UNBASED_UNSIZED_LITERAL_EXPRESSION,
        Self::UNCONNECTED_DRIVE_DIRECTIVE,
        Self::UNDEF_DIRECTIVE,
        Self::UNDEFINE_ALL_DIRECTIVE,
        Self::UNION_TYPE,
        Self::UNIQUENESS_CONSTRAINT,
        Self::UNIT_SCOPE,
        Self::UNTIL_PROPERTY_EXPR,
        Self::UNTIL_WITH_PROPERTY_EXPR,
        Self::UNTYPED,
        Self::USER_DEFINED_NET_DECLARATION,
        Self::VALUE_RANGE_EXPRESSION,
        Self::VARIABLE_DIMENSION,
        Self::VARIABLE_PATTERN,
        Self::VARIABLE_PORT_HEADER,
        Self::VIRTUAL_INTERFACE_TYPE,
        Self::VOID_CASTED_CALL_STATEMENT,
        Self::VOID_TYPE,
        Self::WAIT_FORK_STATEMENT,
        Self::WAIT_ORDER_STATEMENT,
        Self::WAIT_STATEMENT,
        Self::WILDCARD_DIMENSION_SPECIFIER,
        Self::WILDCARD_EQUALITY_EXPRESSION,
        Self::WILDCARD_INEQUALITY_EXPRESSION,
        Self::WILDCARD_LITERAL_EXPRESSION,
        Self::WILDCARD_PATTERN,
        Self::WILDCARD_PORT_CONNECTION,
        Self::WILDCARD_PORT_LIST,
        Self::WILDCARD_UDP_PORT_LIST,
        Self::WITH_CLAUSE,
        Self::WITH_FUNCTION_CLAUSE,
        Self::WITH_FUNCTION_SAMPLE,
        Self::WITHIN_SEQUENCE_EXPR,
        Self::XOR_ASSIGNMENT_EXPRESSION,
    ];
    pub const ALWAYS_BLOCK: Self = Self(8u16);
    pub const ALWAYS_COMB_BLOCK: Self = Self(9u16);
    pub const ALWAYS_FF_BLOCK: Self = Self(10u16);
    pub const ALWAYS_LATCH_BLOCK: Self = Self(11u16);
    pub const AND_ASSIGNMENT_EXPRESSION: Self = Self(12u16);
    pub const AND_PROPERTY_EXPR: Self = Self(13u16);
    pub const AND_SEQUENCE_EXPR: Self = Self(14u16);
    pub const ANONYMOUS_PROGRAM: Self = Self(15u16);
    pub const ANSI_PORT_LIST: Self = Self(16u16);
    pub const ANSI_UDP_PORT_LIST: Self = Self(17u16);
    pub const ARGUMENT_LIST: Self = Self(18u16);
    pub const ARITHMETIC_LEFT_SHIFT_ASSIGNMENT_EXPRESSION: Self = Self(19u16);
    pub const ARITHMETIC_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION: Self = Self(20u16);
    pub const ARITHMETIC_SHIFT_LEFT_EXPRESSION: Self = Self(21u16);
    pub const ARITHMETIC_SHIFT_RIGHT_EXPRESSION: Self = Self(22u16);
    pub const ARRAY_AND_METHOD: Self = Self(23u16);
    pub const ARRAY_OR_METHOD: Self = Self(24u16);
    pub const ARRAY_OR_RANDOMIZE_METHOD_EXPRESSION: Self = Self(25u16);
    pub const ARRAY_UNIQUE_METHOD: Self = Self(26u16);
    pub const ARRAY_XOR_METHOD: Self = Self(27u16);
    pub const ASCENDING_RANGE_SELECT: Self = Self(28u16);
    pub const ASSERTION_ITEM_PORT: Self = Self(30u16);
    pub const ASSERTION_ITEM_PORT_LIST: Self = Self(31u16);
    pub const ASSERT_PROPERTY_STATEMENT: Self = Self(29u16);
    pub const ASSIGNMENT_EXPRESSION: Self = Self(32u16);
    pub const ASSIGNMENT_PATTERN_EXPRESSION: Self = Self(33u16);
    pub const ASSIGNMENT_PATTERN_ITEM: Self = Self(34u16);
    pub const ASSUME_PROPERTY_STATEMENT: Self = Self(35u16);
    pub const ATTRIBUTE_INSTANCE: Self = Self(36u16);
    pub const ATTRIBUTE_SPEC: Self = Self(37u16);
    pub const BAD_EXPRESSION: Self = Self(38u16);
    pub const BEGIN_KEYWORDS_DIRECTIVE: Self = Self(39u16);
    pub const BINARY_AND_EXPRESSION: Self = Self(41u16);
    pub const BINARY_BINS_SELECT_EXPR: Self = Self(42u16);
    pub const BINARY_BLOCK_EVENT_EXPRESSION: Self = Self(43u16);
    pub const BINARY_CONDITIONAL_DIRECTIVE_EXPRESSION: Self = Self(44u16);
    pub const BINARY_EVENT_EXPRESSION: Self = Self(45u16);
    pub const BINARY_OR_EXPRESSION: Self = Self(46u16);
    pub const BINARY_XNOR_EXPRESSION: Self = Self(47u16);
    pub const BINARY_XOR_EXPRESSION: Self = Self(48u16);
    pub const BIND_DIRECTIVE: Self = Self(49u16);
    pub const BIND_TARGET_LIST: Self = Self(50u16);
    pub const BINS_SELECTION: Self = Self(52u16);
    pub const BINS_SELECT_CONDITION_EXPR: Self = Self(51u16);
    pub const BIN_SELECT_WITH_FILTER_EXPR: Self = Self(40u16);
    pub const BIT_SELECT: Self = Self(53u16);
    pub const BIT_TYPE: Self = Self(54u16);
    pub const BLOCKING_EVENT_TRIGGER_STATEMENT: Self = Self(56u16);
    pub const BLOCK_COVERAGE_EVENT: Self = Self(55u16);
    pub const BYTE_TYPE: Self = Self(57u16);
    pub const CASE_EQUALITY_EXPRESSION: Self = Self(59u16);
    pub const CASE_GENERATE: Self = Self(60u16);
    pub const CASE_INEQUALITY_EXPRESSION: Self = Self(61u16);
    pub const CASE_PROPERTY_EXPR: Self = Self(62u16);
    pub const CASE_STATEMENT: Self = Self(63u16);
    pub const CAST_EXPRESSION: Self = Self(64u16);
    pub const CELL_CONFIG_RULE: Self = Self(65u16);
    pub const CELL_DEFINE_DIRECTIVE: Self = Self(66u16);
    pub const CHARGE_STRENGTH: Self = Self(67u16);
    pub const CHECKER_DATA_DECLARATION: Self = Self(68u16);
    pub const CHECKER_DECLARATION: Self = Self(69u16);
    pub const CHECKER_INSTANCE_STATEMENT: Self = Self(70u16);
    pub const CHECKER_INSTANTIATION: Self = Self(71u16);
    pub const CLASS_DECLARATION: Self = Self(72u16);
    pub const CLASS_METHOD_DECLARATION: Self = Self(73u16);
    pub const CLASS_METHOD_PROTOTYPE: Self = Self(74u16);
    pub const CLASS_NAME: Self = Self(75u16);
    pub const CLASS_PROPERTY_DECLARATION: Self = Self(76u16);
    pub const CLASS_SPECIFIER: Self = Self(77u16);
    pub const CLOCKING_DECLARATION: Self = Self(78u16);
    pub const CLOCKING_DIRECTION: Self = Self(79u16);
    pub const CLOCKING_ITEM: Self = Self(80u16);
    pub const CLOCKING_PROPERTY_EXPR: Self = Self(81u16);
    pub const CLOCKING_SEQUENCE_EXPR: Self = Self(82u16);
    pub const CLOCKING_SKEW: Self = Self(83u16);
    pub const COLON_EXPRESSION_CLAUSE: Self = Self(84u16);
    pub const COMPILATION_UNIT: Self = Self(85u16);
    pub const CONCATENATION_EXPRESSION: Self = Self(86u16);
    pub const CONCURRENT_ASSERTION_MEMBER: Self = Self(87u16);
    pub const CONDITIONAL_CONSTRAINT: Self = Self(88u16);
    pub const CONDITIONAL_EXPRESSION: Self = Self(89u16);
    pub const CONDITIONAL_PATH_DECLARATION: Self = Self(90u16);
    pub const CONDITIONAL_PATTERN: Self = Self(91u16);
    pub const CONDITIONAL_PREDICATE: Self = Self(92u16);
    pub const CONDITIONAL_PROPERTY_EXPR: Self = Self(93u16);
    pub const CONDITIONAL_STATEMENT: Self = Self(94u16);
    pub const CONFIG_CELL_IDENTIFIER: Self = Self(95u16);
    pub const CONFIG_DECLARATION: Self = Self(96u16);
    pub const CONFIG_INSTANCE_IDENTIFIER: Self = Self(97u16);
    pub const CONFIG_LIBLIST: Self = Self(98u16);
    pub const CONFIG_USE_CLAUSE: Self = Self(99u16);
    pub const CONSTRAINT_BLOCK: Self = Self(100u16);
    pub const CONSTRAINT_DECLARATION: Self = Self(101u16);
    pub const CONSTRAINT_PROTOTYPE: Self = Self(102u16);
    pub const CONSTRUCTOR_NAME: Self = Self(103u16);
    pub const CONTINUOUS_ASSIGN: Self = Self(104u16);
    pub const COPY_CLASS_EXPRESSION: Self = Self(105u16);
    pub const COVERAGE_BINS: Self = Self(109u16);
    pub const COVERAGE_BINS_ARRAY_SIZE: Self = Self(110u16);
    pub const COVERAGE_IFF_CLAUSE: Self = Self(111u16);
    pub const COVERAGE_OPTION: Self = Self(112u16);
    pub const COVERGROUP_DECLARATION: Self = Self(113u16);
    pub const COVERPOINT: Self = Self(114u16);
    pub const COVER_CROSS: Self = Self(106u16);
    pub const COVER_PROPERTY_STATEMENT: Self = Self(107u16);
    pub const COVER_SEQUENCE_STATEMENT: Self = Self(108u16);
    pub const CYCLE_DELAY: Self = Self(115u16);
    pub const C_HANDLE_TYPE: Self = Self(58u16);
    pub const DATA_DECLARATION: Self = Self(118u16);
    pub const DECLARATOR: Self = Self(119u16);
    pub const DEFAULT_CASE_ITEM: Self = Self(122u16);
    pub const DEFAULT_CLOCKING_REFERENCE: Self = Self(123u16);
    pub const DEFAULT_CONFIG_RULE: Self = Self(124u16);
    pub const DEFAULT_COVERAGE_BIN_INITIALIZER: Self = Self(125u16);
    pub const DEFAULT_DECAY_TIME_DIRECTIVE: Self = Self(126u16);
    pub const DEFAULT_DISABLE_DECLARATION: Self = Self(127u16);
    pub const DEFAULT_DIST_ITEM: Self = Self(128u16);
    pub const DEFAULT_EXTENDS_CLAUSE_ARG: Self = Self(129u16);
    pub const DEFAULT_FUNCTION_PORT: Self = Self(130u16);
    pub const DEFAULT_NET_TYPE_DIRECTIVE: Self = Self(131u16);
    pub const DEFAULT_PATTERN_KEY_EXPRESSION: Self = Self(132u16);
    pub const DEFAULT_PROPERTY_CASE_ITEM: Self = Self(133u16);
    pub const DEFAULT_RS_CASE_ITEM: Self = Self(134u16);
    pub const DEFAULT_SKEW_ITEM: Self = Self(135u16);
    pub const DEFAULT_TRIREG_STRENGTH_DIRECTIVE: Self = Self(136u16);
    pub const DEFERRED_ASSERTION: Self = Self(137u16);
    pub const DEFINE_DIRECTIVE: Self = Self(138u16);
    pub const DEF_PARAM: Self = Self(120u16);
    pub const DEF_PARAM_ASSIGNMENT: Self = Self(121u16);
    pub const DELAYED_SEQUENCE_ELEMENT: Self = Self(145u16);
    pub const DELAYED_SEQUENCE_EXPR: Self = Self(146u16);
    pub const DELAY_3: Self = Self(139u16);
    pub const DELAY_CONTROL: Self = Self(140u16);
    pub const DELAY_MODE_DISTRIBUTED_DIRECTIVE: Self = Self(141u16);
    pub const DELAY_MODE_PATH_DIRECTIVE: Self = Self(142u16);
    pub const DELAY_MODE_UNIT_DIRECTIVE: Self = Self(143u16);
    pub const DELAY_MODE_ZERO_DIRECTIVE: Self = Self(144u16);
    pub const DESCENDING_RANGE_SELECT: Self = Self(147u16);
    pub const DISABLE_CONSTRAINT: Self = Self(148u16);
    pub const DISABLE_FORK_STATEMENT: Self = Self(149u16);
    pub const DISABLE_IFF: Self = Self(150u16);
    pub const DISABLE_STATEMENT: Self = Self(151u16);
    pub const DIST_CONSTRAINT_LIST: Self = Self(152u16);
    pub const DIST_ITEM: Self = Self(153u16);
    pub const DIST_WEIGHT: Self = Self(154u16);
    pub const DIVIDER_CLAUSE: Self = Self(157u16);
    pub const DIVIDE_ASSIGNMENT_EXPRESSION: Self = Self(155u16);
    pub const DIVIDE_EXPRESSION: Self = Self(156u16);
    pub const DOT_MEMBER_CLAUSE: Self = Self(159u16);
    pub const DO_WHILE_STATEMENT: Self = Self(158u16);
    pub const DPI_EXPORT: Self = Self(116u16);
    pub const DPI_IMPORT: Self = Self(117u16);
    pub const DRIVE_STRENGTH: Self = Self(160u16);
    pub const EDGE_CONTROL_SPECIFIER: Self = Self(161u16);
    pub const EDGE_DESCRIPTOR: Self = Self(162u16);
    pub const EDGE_SENSITIVE_PATH_SUFFIX: Self = Self(163u16);
    pub const ELAB_SYSTEM_TASK: Self = Self(164u16);
    pub const ELEMENT_SELECT: Self = Self(165u16);
    pub const ELEMENT_SELECT_EXPRESSION: Self = Self(166u16);
    pub const ELSE_CLAUSE: Self = Self(168u16);
    pub const ELSE_CONSTRAINT_CLAUSE: Self = Self(169u16);
    pub const ELSE_DIRECTIVE: Self = Self(170u16);
    pub const ELSE_PROPERTY_CLAUSE: Self = Self(171u16);
    pub const ELS_IF_DIRECTIVE: Self = Self(167u16);
    pub const EMPTY_ARGUMENT: Self = Self(172u16);
    pub const EMPTY_IDENTIFIER_NAME: Self = Self(173u16);
    pub const EMPTY_MEMBER: Self = Self(174u16);
    pub const EMPTY_NON_ANSI_PORT: Self = Self(175u16);
    pub const EMPTY_PORT_CONNECTION: Self = Self(176u16);
    pub const EMPTY_QUEUE_EXPRESSION: Self = Self(177u16);
    pub const EMPTY_STATEMENT: Self = Self(178u16);
    pub const EMPTY_TIMING_CHECK_ARG: Self = Self(179u16);
    pub const END_CELL_DEFINE_DIRECTIVE: Self = Self(180u16);
    pub const END_IF_DIRECTIVE: Self = Self(181u16);
    pub const END_KEYWORDS_DIRECTIVE: Self = Self(182u16);
    pub const END_PROTECTED_DIRECTIVE: Self = Self(184u16);
    pub const END_PROTECT_DIRECTIVE: Self = Self(183u16);
    pub const ENUM_TYPE: Self = Self(185u16);
    pub const EQUALITY_EXPRESSION: Self = Self(186u16);
    pub const EQUALS_ASSERTION_ARG_CLAUSE: Self = Self(187u16);
    pub const EQUALS_TYPE_CLAUSE: Self = Self(188u16);
    pub const EQUALS_VALUE_CLAUSE: Self = Self(189u16);
    pub const EVENT_CONTROL: Self = Self(190u16);
    pub const EVENT_CONTROL_WITH_EXPRESSION: Self = Self(191u16);
    pub const EVENT_TYPE: Self = Self(192u16);
    pub const EXPECT_PROPERTY_STATEMENT: Self = Self(193u16);
    pub const EXPLICIT_ANSI_PORT: Self = Self(194u16);
    pub const EXPLICIT_NON_ANSI_PORT: Self = Self(195u16);
    pub const EXPRESSION_CONSTRAINT: Self = Self(196u16);
    pub const EXPRESSION_COVERAGE_BIN_INITIALIZER: Self = Self(197u16);
    pub const EXPRESSION_OR_DIST: Self = Self(198u16);
    pub const EXPRESSION_PATTERN: Self = Self(199u16);
    pub const EXPRESSION_STATEMENT: Self = Self(200u16);
    pub const EXPRESSION_TIMING_CHECK_ARG: Self = Self(201u16);
    pub const EXTENDS_CLAUSE: Self = Self(202u16);
    pub const EXTERN_INTERFACE_METHOD: Self = Self(203u16);
    pub const EXTERN_MODULE_DECL: Self = Self(204u16);
    pub const EXTERN_UDP_DECL: Self = Self(205u16);
    pub const FILE_PATH_SPEC: Self = Self(206u16);
    pub const FINAL_BLOCK: Self = Self(207u16);
    pub const FIRST_MATCH_SEQUENCE_EXPR: Self = Self(208u16);
    pub const FOLLOWED_BY_PROPERTY_EXPR: Self = Self(209u16);
    pub const FOREACH_LOOP_LIST: Self = Self(212u16);
    pub const FOREACH_LOOP_STATEMENT: Self = Self(213u16);
    pub const FOREVER_STATEMENT: Self = Self(214u16);
    pub const FORWARD_TYPEDEF_DECLARATION: Self = Self(216u16);
    pub const FORWARD_TYPE_RESTRICTION: Self = Self(215u16);
    pub const FOR_LOOP_STATEMENT: Self = Self(210u16);
    pub const FOR_VARIABLE_DECLARATION: Self = Self(211u16);
    pub const FUNCTION_DECLARATION: Self = Self(217u16);
    pub const FUNCTION_PORT: Self = Self(218u16);
    pub const FUNCTION_PORT_LIST: Self = Self(219u16);
    pub const FUNCTION_PROTOTYPE: Self = Self(220u16);
    pub const GENERATE_BLOCK: Self = Self(221u16);
    pub const GENERATE_REGION: Self = Self(222u16);
    pub const GENVAR_DECLARATION: Self = Self(223u16);
    pub const GREATER_THAN_EQUAL_EXPRESSION: Self = Self(224u16);
    pub const GREATER_THAN_EXPRESSION: Self = Self(225u16);
    pub const HIERARCHICAL_INSTANCE: Self = Self(226u16);
    pub const HIERARCHY_INSTANTIATION: Self = Self(227u16);
    pub const IDENTIFIER_NAME: Self = Self(229u16);
    pub const IDENTIFIER_SELECT_NAME: Self = Self(230u16);
    pub const ID_WITH_EXPR_COVERAGE_BIN_INITIALIZER: Self = Self(228u16);
    pub const IFF_EVENT_CLAUSE: Self = Self(235u16);
    pub const IFF_PROPERTY_EXPR: Self = Self(236u16);
    pub const IF_DEF_DIRECTIVE: Self = Self(231u16);
    pub const IF_GENERATE: Self = Self(232u16);
    pub const IF_NONE_PATH_DECLARATION: Self = Self(234u16);
    pub const IF_N_DEF_DIRECTIVE: Self = Self(233u16);
    pub const IMMEDIATE_ASSERTION_MEMBER: Self = Self(238u16);
    pub const IMMEDIATE_ASSERT_STATEMENT: Self = Self(237u16);
    pub const IMMEDIATE_ASSUME_STATEMENT: Self = Self(239u16);
    pub const IMMEDIATE_COVER_STATEMENT: Self = Self(240u16);
    pub const IMPLEMENTS_CLAUSE: Self = Self(241u16);
    pub const IMPLICATION_CONSTRAINT: Self = Self(242u16);
    pub const IMPLICATION_PROPERTY_EXPR: Self = Self(243u16);
    pub const IMPLICIT_ANSI_PORT: Self = Self(244u16);
    pub const IMPLICIT_EVENT_CONTROL: Self = Self(245u16);
    pub const IMPLICIT_NON_ANSI_PORT: Self = Self(246u16);
    pub const IMPLICIT_TYPE: Self = Self(247u16);
    pub const IMPLIES_PROPERTY_EXPR: Self = Self(248u16);
    pub const INCLUDE_DIRECTIVE: Self = Self(249u16);
    pub const INEQUALITY_EXPRESSION: Self = Self(250u16);
    pub const INITIAL_BLOCK: Self = Self(251u16);
    pub const INSIDE_EXPRESSION: Self = Self(252u16);
    pub const INSTANCE_CONFIG_RULE: Self = Self(253u16);
    pub const INSTANCE_NAME: Self = Self(254u16);
    pub const INTEGER_LITERAL_EXPRESSION: Self = Self(256u16);
    pub const INTEGER_TYPE: Self = Self(257u16);
    pub const INTEGER_VECTOR_EXPRESSION: Self = Self(258u16);
    pub const INTERFACE_DECLARATION: Self = Self(259u16);
    pub const INTERFACE_HEADER: Self = Self(260u16);
    pub const INTERFACE_PORT_HEADER: Self = Self(261u16);
    pub const INTERSECT_CLAUSE: Self = Self(262u16);
    pub const INTERSECT_SEQUENCE_EXPR: Self = Self(263u16);
    pub const INT_TYPE: Self = Self(255u16);
    pub const INVOCATION_EXPRESSION: Self = Self(264u16);
    pub const JUMP_STATEMENT: Self = Self(265u16);
    pub const LESS_THAN_EQUAL_EXPRESSION: Self = Self(266u16);
    pub const LESS_THAN_EXPRESSION: Self = Self(267u16);
    pub const LET_DECLARATION: Self = Self(268u16);
    pub const LIBRARY_DECLARATION: Self = Self(269u16);
    pub const LIBRARY_INCLUDE_STATEMENT: Self = Self(271u16);
    pub const LIBRARY_INC_DIR_CLAUSE: Self = Self(270u16);
    pub const LIBRARY_MAP: Self = Self(272u16);
    pub const LINE_DIRECTIVE: Self = Self(273u16);
    pub const LOCAL_SCOPE: Self = Self(274u16);
    pub const LOCAL_VARIABLE_DECLARATION: Self = Self(275u16);
    pub const LOGICAL_AND_EXPRESSION: Self = Self(277u16);
    pub const LOGICAL_EQUIVALENCE_EXPRESSION: Self = Self(278u16);
    pub const LOGICAL_IMPLICATION_EXPRESSION: Self = Self(279u16);
    pub const LOGICAL_LEFT_SHIFT_ASSIGNMENT_EXPRESSION: Self = Self(280u16);
    pub const LOGICAL_OR_EXPRESSION: Self = Self(281u16);
    pub const LOGICAL_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION: Self = Self(282u16);
    pub const LOGICAL_SHIFT_LEFT_EXPRESSION: Self = Self(283u16);
    pub const LOGICAL_SHIFT_RIGHT_EXPRESSION: Self = Self(284u16);
    pub const LOGIC_TYPE: Self = Self(276u16);
    pub const LONG_INT_TYPE: Self = Self(285u16);
    pub const LOOP_CONSTRAINT: Self = Self(286u16);
    pub const LOOP_GENERATE: Self = Self(287u16);
    pub const LOOP_STATEMENT: Self = Self(288u16);
    pub const MACRO_ACTUAL_ARGUMENT: Self = Self(289u16);
    pub const MACRO_ACTUAL_ARGUMENT_LIST: Self = Self(290u16);
    pub const MACRO_ARGUMENT_DEFAULT: Self = Self(291u16);
    pub const MACRO_FORMAL_ARGUMENT: Self = Self(292u16);
    pub const MACRO_FORMAL_ARGUMENT_LIST: Self = Self(293u16);
    pub const MACRO_USAGE: Self = Self(294u16);
    pub const MATCHES_CLAUSE: Self = Self(295u16);
    pub const MEMBER_ACCESS_EXPRESSION: Self = Self(296u16);
    pub const MIN_TYP_MAX_EXPRESSION: Self = Self(297u16);
    pub const MODPORT_CLOCKING_PORT: Self = Self(300u16);
    pub const MODPORT_DECLARATION: Self = Self(301u16);
    pub const MODPORT_EXPLICIT_PORT: Self = Self(302u16);
    pub const MODPORT_ITEM: Self = Self(303u16);
    pub const MODPORT_NAMED_PORT: Self = Self(304u16);
    pub const MODPORT_SIMPLE_PORT_LIST: Self = Self(305u16);
    pub const MODPORT_SUBROUTINE_PORT: Self = Self(306u16);
    pub const MODPORT_SUBROUTINE_PORT_LIST: Self = Self(307u16);
    pub const MODULE_DECLARATION: Self = Self(308u16);
    pub const MODULE_HEADER: Self = Self(309u16);
    pub const MOD_ASSIGNMENT_EXPRESSION: Self = Self(298u16);
    pub const MOD_EXPRESSION: Self = Self(299u16);
    pub const MULTIPLE_CONCATENATION_EXPRESSION: Self = Self(310u16);
    pub const MULTIPLY_ASSIGNMENT_EXPRESSION: Self = Self(311u16);
    pub const MULTIPLY_EXPRESSION: Self = Self(312u16);
    pub const NAMED_ARGUMENT: Self = Self(314u16);
    pub const NAMED_BLOCK_CLAUSE: Self = Self(315u16);
    pub const NAMED_CONDITIONAL_DIRECTIVE_EXPRESSION: Self = Self(316u16);
    pub const NAMED_LABEL: Self = Self(317u16);
    pub const NAMED_PARAM_ASSIGNMENT: Self = Self(318u16);
    pub const NAMED_PORT_CONNECTION: Self = Self(319u16);
    pub const NAMED_STRUCTURE_PATTERN_MEMBER: Self = Self(320u16);
    pub const NAMED_TYPE: Self = Self(321u16);
    pub const NAME_VALUE_PRAGMA_EXPRESSION: Self = Self(313u16);
    pub const NET_ALIAS: Self = Self(322u16);
    pub const NET_DECLARATION: Self = Self(323u16);
    pub const NET_PORT_HEADER: Self = Self(324u16);
    pub const NET_TYPE_DECLARATION: Self = Self(325u16);
    pub const NEW_ARRAY_EXPRESSION: Self = Self(326u16);
    pub const NEW_CLASS_EXPRESSION: Self = Self(327u16);
    pub const NONBLOCKING_ASSIGNMENT_EXPRESSION: Self = Self(331u16);
    pub const NONBLOCKING_EVENT_TRIGGER_STATEMENT: Self = Self(332u16);
    pub const NON_ANSI_PORT_LIST: Self = Self(329u16);
    pub const NON_ANSI_UDP_PORT_LIST: Self = Self(330u16);
    pub const NO_UNCONNECTED_DRIVE_DIRECTIVE: Self = Self(328u16);
    pub const NULL_LITERAL_EXPRESSION: Self = Self(333u16);
    pub const NUMBER_PRAGMA_EXPRESSION: Self = Self(334u16);
    pub const ONE_STEP_DELAY: Self = Self(335u16);
    pub const ORDERED_ARGUMENT: Self = Self(339u16);
    pub const ORDERED_PARAM_ASSIGNMENT: Self = Self(340u16);
    pub const ORDERED_PORT_CONNECTION: Self = Self(341u16);
    pub const ORDERED_STRUCTURE_PATTERN_MEMBER: Self = Self(342u16);
    pub const OR_ASSIGNMENT_EXPRESSION: Self = Self(336u16);
    pub const OR_PROPERTY_EXPR: Self = Self(337u16);
    pub const OR_SEQUENCE_EXPR: Self = Self(338u16);
    pub const PACKAGE_DECLARATION: Self = Self(343u16);
    pub const PACKAGE_EXPORT_ALL_DECLARATION: Self = Self(344u16);
    pub const PACKAGE_EXPORT_DECLARATION: Self = Self(345u16);
    pub const PACKAGE_HEADER: Self = Self(346u16);
    pub const PACKAGE_IMPORT_DECLARATION: Self = Self(347u16);
    pub const PACKAGE_IMPORT_ITEM: Self = Self(348u16);
    pub const PARALLEL_BLOCK_STATEMENT: Self = Self(349u16);
    pub const PARAMETER_DECLARATION: Self = Self(350u16);
    pub const PARAMETER_DECLARATION_STATEMENT: Self = Self(351u16);
    pub const PARAMETER_PORT_LIST: Self = Self(352u16);
    pub const PARAMETER_VALUE_ASSIGNMENT: Self = Self(353u16);
    pub const PARENTHESIZED_BINS_SELECT_EXPR: Self = Self(356u16);
    pub const PARENTHESIZED_CONDITIONAL_DIRECTIVE_EXPRESSION: Self = Self(357u16);
    pub const PARENTHESIZED_EVENT_EXPRESSION: Self = Self(358u16);
    pub const PARENTHESIZED_EXPRESSION: Self = Self(359u16);
    pub const PARENTHESIZED_PATTERN: Self = Self(360u16);
    pub const PARENTHESIZED_PROPERTY_EXPR: Self = Self(361u16);
    pub const PARENTHESIZED_SEQUENCE_EXPR: Self = Self(362u16);
    pub const PAREN_EXPRESSION_LIST: Self = Self(354u16);
    pub const PAREN_PRAGMA_EXPRESSION: Self = Self(355u16);
    pub const PATH_DECLARATION: Self = Self(363u16);
    pub const PATH_DESCRIPTION: Self = Self(364u16);
    pub const PATTERN_CASE_ITEM: Self = Self(365u16);
    pub const PORT_CONCATENATION: Self = Self(366u16);
    pub const PORT_DECLARATION: Self = Self(367u16);
    pub const PORT_REFERENCE: Self = Self(368u16);
    pub const POSTDECREMENT_EXPRESSION: Self = Self(369u16);
    pub const POSTINCREMENT_EXPRESSION: Self = Self(370u16);
    pub const POWER_EXPRESSION: Self = Self(371u16);
    pub const PRAGMA_DIRECTIVE: Self = Self(372u16);
    pub const PRIMARY_BLOCK_EVENT_EXPRESSION: Self = Self(373u16);
    pub const PRIMITIVE_INSTANTIATION: Self = Self(374u16);
    pub const PROCEDURAL_ASSIGN_STATEMENT: Self = Self(375u16);
    pub const PROCEDURAL_DEASSIGN_STATEMENT: Self = Self(376u16);
    pub const PROCEDURAL_FORCE_STATEMENT: Self = Self(377u16);
    pub const PROCEDURAL_RELEASE_STATEMENT: Self = Self(378u16);
    pub const PRODUCTION: Self = Self(379u16);
    pub const PROGRAM_DECLARATION: Self = Self(380u16);
    pub const PROGRAM_HEADER: Self = Self(381u16);
    pub const PROPERTY_DECLARATION: Self = Self(382u16);
    pub const PROPERTY_SPEC: Self = Self(383u16);
    pub const PROPERTY_TYPE: Self = Self(384u16);
    pub const PROTECTED_DIRECTIVE: Self = Self(386u16);
    pub const PROTECT_DIRECTIVE: Self = Self(385u16);
    pub const PULL_STRENGTH: Self = Self(387u16);
    pub const PULSE_STYLE_DECLARATION: Self = Self(388u16);
    pub const QUEUE_DIMENSION_SPECIFIER: Self = Self(389u16);
    pub const RAND_CASE_ITEM: Self = Self(390u16);
    pub const RAND_CASE_STATEMENT: Self = Self(391u16);
    pub const RAND_JOIN_CLAUSE: Self = Self(392u16);
    pub const RAND_SEQUENCE_STATEMENT: Self = Self(393u16);
    pub const RANGE_COVERAGE_BIN_INITIALIZER: Self = Self(394u16);
    pub const RANGE_DIMENSION_SPECIFIER: Self = Self(395u16);
    pub const RANGE_LIST: Self = Self(396u16);
    pub const REAL_LITERAL_EXPRESSION: Self = Self(397u16);
    pub const REAL_TIME_TYPE: Self = Self(398u16);
    pub const REAL_TYPE: Self = Self(399u16);
    pub const REG_TYPE: Self = Self(400u16);
    pub const REPEATED_EVENT_CONTROL: Self = Self(401u16);
    pub const REPLICATED_ASSIGNMENT_PATTERN: Self = Self(402u16);
    pub const RESET_ALL_DIRECTIVE: Self = Self(403u16);
    pub const RESTRICT_PROPERTY_STATEMENT: Self = Self(404u16);
    pub const RETURN_STATEMENT: Self = Self(405u16);
    pub const ROOT_SCOPE: Self = Self(406u16);
    pub const RS_CASE: Self = Self(407u16);
    pub const RS_CODE_BLOCK: Self = Self(408u16);
    pub const RS_ELSE_CLAUSE: Self = Self(409u16);
    pub const RS_IF_ELSE: Self = Self(410u16);
    pub const RS_PROD_ITEM: Self = Self(411u16);
    pub const RS_REPEAT: Self = Self(412u16);
    pub const RS_RULE: Self = Self(413u16);
    pub const RS_WEIGHT_CLAUSE: Self = Self(414u16);
    pub const SCOPED_NAME: Self = Self(417u16);
    pub const SEPARATED_LIST: Self = Self(3u16);
    pub const SEQUENCE_DECLARATION: Self = Self(418u16);
    pub const SEQUENCE_MATCH_LIST: Self = Self(419u16);
    pub const SEQUENCE_REPETITION: Self = Self(420u16);
    pub const SEQUENCE_TYPE: Self = Self(421u16);
    pub const SEQUENTIAL_BLOCK_STATEMENT: Self = Self(422u16);
    pub const SHORT_INT_TYPE: Self = Self(423u16);
    pub const SHORT_REAL_TYPE: Self = Self(424u16);
    pub const SIGNAL_EVENT_EXPRESSION: Self = Self(425u16);
    pub const SIGNED_CAST_EXPRESSION: Self = Self(426u16);
    pub const SIMPLE_ASSIGNMENT_PATTERN: Self = Self(427u16);
    pub const SIMPLE_BINS_SELECT_EXPR: Self = Self(428u16);
    pub const SIMPLE_PATH_SUFFIX: Self = Self(429u16);
    pub const SIMPLE_PRAGMA_EXPRESSION: Self = Self(430u16);
    pub const SIMPLE_PROPERTY_EXPR: Self = Self(431u16);
    pub const SIMPLE_RANGE_SELECT: Self = Self(432u16);
    pub const SIMPLE_SEQUENCE_EXPR: Self = Self(433u16);
    pub const SOLVE_BEFORE_CONSTRAINT: Self = Self(434u16);
    pub const SPECIFY_BLOCK: Self = Self(435u16);
    pub const SPECPARAM_DECLARATION: Self = Self(436u16);
    pub const SPECPARAM_DECLARATOR: Self = Self(437u16);
    pub const STANDARD_CASE_ITEM: Self = Self(438u16);
    pub const STANDARD_PROPERTY_CASE_ITEM: Self = Self(439u16);
    pub const STANDARD_RS_CASE_ITEM: Self = Self(440u16);
    pub const STREAMING_CONCATENATION_EXPRESSION: Self = Self(443u16);
    pub const STREAM_EXPRESSION: Self = Self(441u16);
    pub const STREAM_EXPRESSION_WITH_RANGE: Self = Self(442u16);
    pub const STRING_LITERAL_EXPRESSION: Self = Self(444u16);
    pub const STRING_TYPE: Self = Self(445u16);
    pub const STRONG_WEAK_PROPERTY_EXPR: Self = Self(446u16);
    pub const STRUCTURED_ASSIGNMENT_PATTERN: Self = Self(450u16);
    pub const STRUCTURE_PATTERN: Self = Self(449u16);
    pub const STRUCT_TYPE: Self = Self(447u16);
    pub const STRUCT_UNION_MEMBER: Self = Self(448u16);
    pub const SUBTRACT_ASSIGNMENT_EXPRESSION: Self = Self(451u16);
    pub const SUBTRACT_EXPRESSION: Self = Self(452u16);
    pub const SUPER_HANDLE: Self = Self(453u16);
    pub const SUPER_NEW_DEFAULTED_ARGS_EXPRESSION: Self = Self(454u16);
    pub const SYNTAX_LIST: Self = Self(1u16);
    pub const SYSTEM_NAME: Self = Self(455u16);
    pub const SYSTEM_TIMING_CHECK: Self = Self(456u16);
    pub const S_UNTIL_PROPERTY_EXPR: Self = Self(415u16);
    pub const S_UNTIL_WITH_PROPERTY_EXPR: Self = Self(416u16);
    pub const TAGGED_PATTERN: Self = Self(457u16);
    pub const TAGGED_UNION_EXPRESSION: Self = Self(458u16);
    pub const TASK_DECLARATION: Self = Self(459u16);
    pub const THIS_HANDLE: Self = Self(460u16);
    pub const THROUGHOUT_SEQUENCE_EXPR: Self = Self(461u16);
    pub const TIME_LITERAL_EXPRESSION: Self = Self(462u16);
    pub const TIME_SCALE_DIRECTIVE: Self = Self(463u16);
    pub const TIME_TYPE: Self = Self(464u16);
    pub const TIME_UNITS_DECLARATION: Self = Self(465u16);
    pub const TIMING_CHECK_EVENT_ARG: Self = Self(466u16);
    pub const TIMING_CHECK_EVENT_CONDITION: Self = Self(467u16);
    pub const TIMING_CONTROL_EXPRESSION: Self = Self(468u16);
    pub const TIMING_CONTROL_STATEMENT: Self = Self(469u16);
    pub const TOKEN_LIST: Self = Self(2u16);
    pub const TRANS_LIST_COVERAGE_BIN_INITIALIZER: Self = Self(470u16);
    pub const TRANS_RANGE: Self = Self(471u16);
    pub const TRANS_REPEAT_RANGE: Self = Self(472u16);
    pub const TRANS_SET: Self = Self(473u16);
    pub const TYPEDEF_DECLARATION: Self = Self(477u16);
    pub const TYPE_ASSIGNMENT: Self = Self(474u16);
    pub const TYPE_PARAMETER_DECLARATION: Self = Self(475u16);
    pub const TYPE_REFERENCE: Self = Self(476u16);
    pub const UDP_BODY: Self = Self(478u16);
    pub const UDP_DECLARATION: Self = Self(479u16);
    pub const UDP_EDGE_FIELD: Self = Self(480u16);
    pub const UDP_ENTRY: Self = Self(481u16);
    pub const UDP_INITIAL_STMT: Self = Self(482u16);
    pub const UDP_INPUT_PORT_DECL: Self = Self(483u16);
    pub const UDP_OUTPUT_PORT_DECL: Self = Self(484u16);
    pub const UDP_SIMPLE_FIELD: Self = Self(485u16);
    pub const UNARY_BINS_SELECT_EXPR: Self = Self(486u16);
    pub const UNARY_BITWISE_AND_EXPRESSION: Self = Self(487u16);
    pub const UNARY_BITWISE_NAND_EXPRESSION: Self = Self(488u16);
    pub const UNARY_BITWISE_NOR_EXPRESSION: Self = Self(489u16);
    pub const UNARY_BITWISE_NOT_EXPRESSION: Self = Self(490u16);
    pub const UNARY_BITWISE_OR_EXPRESSION: Self = Self(491u16);
    pub const UNARY_BITWISE_XNOR_EXPRESSION: Self = Self(492u16);
    pub const UNARY_BITWISE_XOR_EXPRESSION: Self = Self(493u16);
    pub const UNARY_CONDITIONAL_DIRECTIVE_EXPRESSION: Self = Self(494u16);
    pub const UNARY_LOGICAL_NOT_EXPRESSION: Self = Self(495u16);
    pub const UNARY_MINUS_EXPRESSION: Self = Self(496u16);
    pub const UNARY_PLUS_EXPRESSION: Self = Self(497u16);
    pub const UNARY_PREDECREMENT_EXPRESSION: Self = Self(498u16);
    pub const UNARY_PREINCREMENT_EXPRESSION: Self = Self(499u16);
    pub const UNARY_PROPERTY_EXPR: Self = Self(500u16);
    pub const UNARY_SELECT_PROPERTY_EXPR: Self = Self(501u16);
    pub const UNBASED_UNSIZED_LITERAL_EXPRESSION: Self = Self(502u16);
    pub const UNCONNECTED_DRIVE_DIRECTIVE: Self = Self(503u16);
    pub const UNDEFINE_ALL_DIRECTIVE: Self = Self(505u16);
    pub const UNDEF_DIRECTIVE: Self = Self(504u16);
    pub const UNION_TYPE: Self = Self(506u16);
    pub const UNIQUENESS_CONSTRAINT: Self = Self(507u16);
    pub const UNIT_SCOPE: Self = Self(508u16);
    pub const UNKNOWN: Self = Self(0u16);
    pub const UNTIL_PROPERTY_EXPR: Self = Self(509u16);
    pub const UNTIL_WITH_PROPERTY_EXPR: Self = Self(510u16);
    pub const UNTYPED: Self = Self(511u16);
    pub const USER_DEFINED_NET_DECLARATION: Self = Self(512u16);
    pub const VALUE_RANGE_EXPRESSION: Self = Self(513u16);
    pub const VARIABLE_DIMENSION: Self = Self(514u16);
    pub const VARIABLE_PATTERN: Self = Self(515u16);
    pub const VARIABLE_PORT_HEADER: Self = Self(516u16);
    pub const VIRTUAL_INTERFACE_TYPE: Self = Self(517u16);
    pub const VOID_CASTED_CALL_STATEMENT: Self = Self(518u16);
    pub const VOID_TYPE: Self = Self(519u16);
    pub const WAIT_FORK_STATEMENT: Self = Self(520u16);
    pub const WAIT_ORDER_STATEMENT: Self = Self(521u16);
    pub const WAIT_STATEMENT: Self = Self(522u16);
    pub const WILDCARD_DIMENSION_SPECIFIER: Self = Self(523u16);
    pub const WILDCARD_EQUALITY_EXPRESSION: Self = Self(524u16);
    pub const WILDCARD_INEQUALITY_EXPRESSION: Self = Self(525u16);
    pub const WILDCARD_LITERAL_EXPRESSION: Self = Self(526u16);
    pub const WILDCARD_PATTERN: Self = Self(527u16);
    pub const WILDCARD_PORT_CONNECTION: Self = Self(528u16);
    pub const WILDCARD_PORT_LIST: Self = Self(529u16);
    pub const WILDCARD_UDP_PORT_LIST: Self = Self(530u16);
    pub const WITHIN_SEQUENCE_EXPR: Self = Self(534u16);
    pub const WITH_CLAUSE: Self = Self(531u16);
    pub const WITH_FUNCTION_CLAUSE: Self = Self(532u16);
    pub const WITH_FUNCTION_SAMPLE: Self = Self(533u16);
    pub const XOR_ASSIGNMENT_EXPRESSION: Self = Self(535u16);

    pub fn is_list(&self) -> bool {
        *self == Self::SYNTAX_LIST || *self == Self::TOKEN_LIST || *self == Self::SEPARATED_LIST
    }

    pub fn from_id(id: u16) -> Self {
        Self(id)
    }
}
impl fmt::Debug for SyntaxKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match *self {
            Self::UNKNOWN => "Unknown",
            Self::SYNTAX_LIST => "SyntaxList",
            Self::TOKEN_LIST => "TokenList",
            Self::SEPARATED_LIST => "SeparatedList",
            Self::ACCEPT_ON_PROPERTY_EXPR => "AcceptOnPropertyExpr",
            Self::ACTION_BLOCK => "ActionBlock",
            Self::ADD_ASSIGNMENT_EXPRESSION => "AddAssignmentExpression",
            Self::ADD_EXPRESSION => "AddExpression",
            Self::ALWAYS_BLOCK => "AlwaysBlock",
            Self::ALWAYS_COMB_BLOCK => "AlwaysCombBlock",
            Self::ALWAYS_FF_BLOCK => "AlwaysFFBlock",
            Self::ALWAYS_LATCH_BLOCK => "AlwaysLatchBlock",
            Self::AND_ASSIGNMENT_EXPRESSION => "AndAssignmentExpression",
            Self::AND_PROPERTY_EXPR => "AndPropertyExpr",
            Self::AND_SEQUENCE_EXPR => "AndSequenceExpr",
            Self::ANONYMOUS_PROGRAM => "AnonymousProgram",
            Self::ANSI_PORT_LIST => "AnsiPortList",
            Self::ANSI_UDP_PORT_LIST => "AnsiUdpPortList",
            Self::ARGUMENT_LIST => "ArgumentList",
            Self::ARITHMETIC_LEFT_SHIFT_ASSIGNMENT_EXPRESSION => {
                "ArithmeticLeftShiftAssignmentExpression"
            }
            Self::ARITHMETIC_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION => {
                "ArithmeticRightShiftAssignmentExpression"
            }
            Self::ARITHMETIC_SHIFT_LEFT_EXPRESSION => "ArithmeticShiftLeftExpression",
            Self::ARITHMETIC_SHIFT_RIGHT_EXPRESSION => "ArithmeticShiftRightExpression",
            Self::ARRAY_AND_METHOD => "ArrayAndMethod",
            Self::ARRAY_OR_METHOD => "ArrayOrMethod",
            Self::ARRAY_OR_RANDOMIZE_METHOD_EXPRESSION => "ArrayOrRandomizeMethodExpression",
            Self::ARRAY_UNIQUE_METHOD => "ArrayUniqueMethod",
            Self::ARRAY_XOR_METHOD => "ArrayXorMethod",
            Self::ASCENDING_RANGE_SELECT => "AscendingRangeSelect",
            Self::ASSERT_PROPERTY_STATEMENT => "AssertPropertyStatement",
            Self::ASSERTION_ITEM_PORT => "AssertionItemPort",
            Self::ASSERTION_ITEM_PORT_LIST => "AssertionItemPortList",
            Self::ASSIGNMENT_EXPRESSION => "AssignmentExpression",
            Self::ASSIGNMENT_PATTERN_EXPRESSION => "AssignmentPatternExpression",
            Self::ASSIGNMENT_PATTERN_ITEM => "AssignmentPatternItem",
            Self::ASSUME_PROPERTY_STATEMENT => "AssumePropertyStatement",
            Self::ATTRIBUTE_INSTANCE => "AttributeInstance",
            Self::ATTRIBUTE_SPEC => "AttributeSpec",
            Self::BAD_EXPRESSION => "BadExpression",
            Self::BEGIN_KEYWORDS_DIRECTIVE => "BeginKeywordsDirective",
            Self::BIN_SELECT_WITH_FILTER_EXPR => "BinSelectWithFilterExpr",
            Self::BINARY_AND_EXPRESSION => "BinaryAndExpression",
            Self::BINARY_BINS_SELECT_EXPR => "BinaryBinsSelectExpr",
            Self::BINARY_BLOCK_EVENT_EXPRESSION => "BinaryBlockEventExpression",
            Self::BINARY_CONDITIONAL_DIRECTIVE_EXPRESSION => "BinaryConditionalDirectiveExpression",
            Self::BINARY_EVENT_EXPRESSION => "BinaryEventExpression",
            Self::BINARY_OR_EXPRESSION => "BinaryOrExpression",
            Self::BINARY_XNOR_EXPRESSION => "BinaryXnorExpression",
            Self::BINARY_XOR_EXPRESSION => "BinaryXorExpression",
            Self::BIND_DIRECTIVE => "BindDirective",
            Self::BIND_TARGET_LIST => "BindTargetList",
            Self::BINS_SELECT_CONDITION_EXPR => "BinsSelectConditionExpr",
            Self::BINS_SELECTION => "BinsSelection",
            Self::BIT_SELECT => "BitSelect",
            Self::BIT_TYPE => "BitType",
            Self::BLOCK_COVERAGE_EVENT => "BlockCoverageEvent",
            Self::BLOCKING_EVENT_TRIGGER_STATEMENT => "BlockingEventTriggerStatement",
            Self::BYTE_TYPE => "ByteType",
            Self::C_HANDLE_TYPE => "CHandleType",
            Self::CASE_EQUALITY_EXPRESSION => "CaseEqualityExpression",
            Self::CASE_GENERATE => "CaseGenerate",
            Self::CASE_INEQUALITY_EXPRESSION => "CaseInequalityExpression",
            Self::CASE_PROPERTY_EXPR => "CasePropertyExpr",
            Self::CASE_STATEMENT => "CaseStatement",
            Self::CAST_EXPRESSION => "CastExpression",
            Self::CELL_CONFIG_RULE => "CellConfigRule",
            Self::CELL_DEFINE_DIRECTIVE => "CellDefineDirective",
            Self::CHARGE_STRENGTH => "ChargeStrength",
            Self::CHECKER_DATA_DECLARATION => "CheckerDataDeclaration",
            Self::CHECKER_DECLARATION => "CheckerDeclaration",
            Self::CHECKER_INSTANCE_STATEMENT => "CheckerInstanceStatement",
            Self::CHECKER_INSTANTIATION => "CheckerInstantiation",
            Self::CLASS_DECLARATION => "ClassDeclaration",
            Self::CLASS_METHOD_DECLARATION => "ClassMethodDeclaration",
            Self::CLASS_METHOD_PROTOTYPE => "ClassMethodPrototype",
            Self::CLASS_NAME => "ClassName",
            Self::CLASS_PROPERTY_DECLARATION => "ClassPropertyDeclaration",
            Self::CLASS_SPECIFIER => "ClassSpecifier",
            Self::CLOCKING_DECLARATION => "ClockingDeclaration",
            Self::CLOCKING_DIRECTION => "ClockingDirection",
            Self::CLOCKING_ITEM => "ClockingItem",
            Self::CLOCKING_PROPERTY_EXPR => "ClockingPropertyExpr",
            Self::CLOCKING_SEQUENCE_EXPR => "ClockingSequenceExpr",
            Self::CLOCKING_SKEW => "ClockingSkew",
            Self::COLON_EXPRESSION_CLAUSE => "ColonExpressionClause",
            Self::COMPILATION_UNIT => "CompilationUnit",
            Self::CONCATENATION_EXPRESSION => "ConcatenationExpression",
            Self::CONCURRENT_ASSERTION_MEMBER => "ConcurrentAssertionMember",
            Self::CONDITIONAL_CONSTRAINT => "ConditionalConstraint",
            Self::CONDITIONAL_EXPRESSION => "ConditionalExpression",
            Self::CONDITIONAL_PATH_DECLARATION => "ConditionalPathDeclaration",
            Self::CONDITIONAL_PATTERN => "ConditionalPattern",
            Self::CONDITIONAL_PREDICATE => "ConditionalPredicate",
            Self::CONDITIONAL_PROPERTY_EXPR => "ConditionalPropertyExpr",
            Self::CONDITIONAL_STATEMENT => "ConditionalStatement",
            Self::CONFIG_CELL_IDENTIFIER => "ConfigCellIdentifier",
            Self::CONFIG_DECLARATION => "ConfigDeclaration",
            Self::CONFIG_INSTANCE_IDENTIFIER => "ConfigInstanceIdentifier",
            Self::CONFIG_LIBLIST => "ConfigLiblist",
            Self::CONFIG_USE_CLAUSE => "ConfigUseClause",
            Self::CONSTRAINT_BLOCK => "ConstraintBlock",
            Self::CONSTRAINT_DECLARATION => "ConstraintDeclaration",
            Self::CONSTRAINT_PROTOTYPE => "ConstraintPrototype",
            Self::CONSTRUCTOR_NAME => "ConstructorName",
            Self::CONTINUOUS_ASSIGN => "ContinuousAssign",
            Self::COPY_CLASS_EXPRESSION => "CopyClassExpression",
            Self::COVER_CROSS => "CoverCross",
            Self::COVER_PROPERTY_STATEMENT => "CoverPropertyStatement",
            Self::COVER_SEQUENCE_STATEMENT => "CoverSequenceStatement",
            Self::COVERAGE_BINS => "CoverageBins",
            Self::COVERAGE_BINS_ARRAY_SIZE => "CoverageBinsArraySize",
            Self::COVERAGE_IFF_CLAUSE => "CoverageIffClause",
            Self::COVERAGE_OPTION => "CoverageOption",
            Self::COVERGROUP_DECLARATION => "CovergroupDeclaration",
            Self::COVERPOINT => "Coverpoint",
            Self::CYCLE_DELAY => "CycleDelay",
            Self::DPI_EXPORT => "DPIExport",
            Self::DPI_IMPORT => "DPIImport",
            Self::DATA_DECLARATION => "DataDeclaration",
            Self::DECLARATOR => "Declarator",
            Self::DEF_PARAM => "DefParam",
            Self::DEF_PARAM_ASSIGNMENT => "DefParamAssignment",
            Self::DEFAULT_CASE_ITEM => "DefaultCaseItem",
            Self::DEFAULT_CLOCKING_REFERENCE => "DefaultClockingReference",
            Self::DEFAULT_CONFIG_RULE => "DefaultConfigRule",
            Self::DEFAULT_COVERAGE_BIN_INITIALIZER => "DefaultCoverageBinInitializer",
            Self::DEFAULT_DECAY_TIME_DIRECTIVE => "DefaultDecayTimeDirective",
            Self::DEFAULT_DISABLE_DECLARATION => "DefaultDisableDeclaration",
            Self::DEFAULT_DIST_ITEM => "DefaultDistItem",
            Self::DEFAULT_EXTENDS_CLAUSE_ARG => "DefaultExtendsClauseArg",
            Self::DEFAULT_FUNCTION_PORT => "DefaultFunctionPort",
            Self::DEFAULT_NET_TYPE_DIRECTIVE => "DefaultNetTypeDirective",
            Self::DEFAULT_PATTERN_KEY_EXPRESSION => "DefaultPatternKeyExpression",
            Self::DEFAULT_PROPERTY_CASE_ITEM => "DefaultPropertyCaseItem",
            Self::DEFAULT_RS_CASE_ITEM => "DefaultRsCaseItem",
            Self::DEFAULT_SKEW_ITEM => "DefaultSkewItem",
            Self::DEFAULT_TRIREG_STRENGTH_DIRECTIVE => "DefaultTriregStrengthDirective",
            Self::DEFERRED_ASSERTION => "DeferredAssertion",
            Self::DEFINE_DIRECTIVE => "DefineDirective",
            Self::DELAY_3 => "Delay3",
            Self::DELAY_CONTROL => "DelayControl",
            Self::DELAY_MODE_DISTRIBUTED_DIRECTIVE => "DelayModeDistributedDirective",
            Self::DELAY_MODE_PATH_DIRECTIVE => "DelayModePathDirective",
            Self::DELAY_MODE_UNIT_DIRECTIVE => "DelayModeUnitDirective",
            Self::DELAY_MODE_ZERO_DIRECTIVE => "DelayModeZeroDirective",
            Self::DELAYED_SEQUENCE_ELEMENT => "DelayedSequenceElement",
            Self::DELAYED_SEQUENCE_EXPR => "DelayedSequenceExpr",
            Self::DESCENDING_RANGE_SELECT => "DescendingRangeSelect",
            Self::DISABLE_CONSTRAINT => "DisableConstraint",
            Self::DISABLE_FORK_STATEMENT => "DisableForkStatement",
            Self::DISABLE_IFF => "DisableIff",
            Self::DISABLE_STATEMENT => "DisableStatement",
            Self::DIST_CONSTRAINT_LIST => "DistConstraintList",
            Self::DIST_ITEM => "DistItem",
            Self::DIST_WEIGHT => "DistWeight",
            Self::DIVIDE_ASSIGNMENT_EXPRESSION => "DivideAssignmentExpression",
            Self::DIVIDE_EXPRESSION => "DivideExpression",
            Self::DIVIDER_CLAUSE => "DividerClause",
            Self::DO_WHILE_STATEMENT => "DoWhileStatement",
            Self::DOT_MEMBER_CLAUSE => "DotMemberClause",
            Self::DRIVE_STRENGTH => "DriveStrength",
            Self::EDGE_CONTROL_SPECIFIER => "EdgeControlSpecifier",
            Self::EDGE_DESCRIPTOR => "EdgeDescriptor",
            Self::EDGE_SENSITIVE_PATH_SUFFIX => "EdgeSensitivePathSuffix",
            Self::ELAB_SYSTEM_TASK => "ElabSystemTask",
            Self::ELEMENT_SELECT => "ElementSelect",
            Self::ELEMENT_SELECT_EXPRESSION => "ElementSelectExpression",
            Self::ELS_IF_DIRECTIVE => "ElsIfDirective",
            Self::ELSE_CLAUSE => "ElseClause",
            Self::ELSE_CONSTRAINT_CLAUSE => "ElseConstraintClause",
            Self::ELSE_DIRECTIVE => "ElseDirective",
            Self::ELSE_PROPERTY_CLAUSE => "ElsePropertyClause",
            Self::EMPTY_ARGUMENT => "EmptyArgument",
            Self::EMPTY_IDENTIFIER_NAME => "EmptyIdentifierName",
            Self::EMPTY_MEMBER => "EmptyMember",
            Self::EMPTY_NON_ANSI_PORT => "EmptyNonAnsiPort",
            Self::EMPTY_PORT_CONNECTION => "EmptyPortConnection",
            Self::EMPTY_QUEUE_EXPRESSION => "EmptyQueueExpression",
            Self::EMPTY_STATEMENT => "EmptyStatement",
            Self::EMPTY_TIMING_CHECK_ARG => "EmptyTimingCheckArg",
            Self::END_CELL_DEFINE_DIRECTIVE => "EndCellDefineDirective",
            Self::END_IF_DIRECTIVE => "EndIfDirective",
            Self::END_KEYWORDS_DIRECTIVE => "EndKeywordsDirective",
            Self::END_PROTECT_DIRECTIVE => "EndProtectDirective",
            Self::END_PROTECTED_DIRECTIVE => "EndProtectedDirective",
            Self::ENUM_TYPE => "EnumType",
            Self::EQUALITY_EXPRESSION => "EqualityExpression",
            Self::EQUALS_ASSERTION_ARG_CLAUSE => "EqualsAssertionArgClause",
            Self::EQUALS_TYPE_CLAUSE => "EqualsTypeClause",
            Self::EQUALS_VALUE_CLAUSE => "EqualsValueClause",
            Self::EVENT_CONTROL => "EventControl",
            Self::EVENT_CONTROL_WITH_EXPRESSION => "EventControlWithExpression",
            Self::EVENT_TYPE => "EventType",
            Self::EXPECT_PROPERTY_STATEMENT => "ExpectPropertyStatement",
            Self::EXPLICIT_ANSI_PORT => "ExplicitAnsiPort",
            Self::EXPLICIT_NON_ANSI_PORT => "ExplicitNonAnsiPort",
            Self::EXPRESSION_CONSTRAINT => "ExpressionConstraint",
            Self::EXPRESSION_COVERAGE_BIN_INITIALIZER => "ExpressionCoverageBinInitializer",
            Self::EXPRESSION_OR_DIST => "ExpressionOrDist",
            Self::EXPRESSION_PATTERN => "ExpressionPattern",
            Self::EXPRESSION_STATEMENT => "ExpressionStatement",
            Self::EXPRESSION_TIMING_CHECK_ARG => "ExpressionTimingCheckArg",
            Self::EXTENDS_CLAUSE => "ExtendsClause",
            Self::EXTERN_INTERFACE_METHOD => "ExternInterfaceMethod",
            Self::EXTERN_MODULE_DECL => "ExternModuleDecl",
            Self::EXTERN_UDP_DECL => "ExternUdpDecl",
            Self::FILE_PATH_SPEC => "FilePathSpec",
            Self::FINAL_BLOCK => "FinalBlock",
            Self::FIRST_MATCH_SEQUENCE_EXPR => "FirstMatchSequenceExpr",
            Self::FOLLOWED_BY_PROPERTY_EXPR => "FollowedByPropertyExpr",
            Self::FOR_LOOP_STATEMENT => "ForLoopStatement",
            Self::FOR_VARIABLE_DECLARATION => "ForVariableDeclaration",
            Self::FOREACH_LOOP_LIST => "ForeachLoopList",
            Self::FOREACH_LOOP_STATEMENT => "ForeachLoopStatement",
            Self::FOREVER_STATEMENT => "ForeverStatement",
            Self::FORWARD_TYPE_RESTRICTION => "ForwardTypeRestriction",
            Self::FORWARD_TYPEDEF_DECLARATION => "ForwardTypedefDeclaration",
            Self::FUNCTION_DECLARATION => "FunctionDeclaration",
            Self::FUNCTION_PORT => "FunctionPort",
            Self::FUNCTION_PORT_LIST => "FunctionPortList",
            Self::FUNCTION_PROTOTYPE => "FunctionPrototype",
            Self::GENERATE_BLOCK => "GenerateBlock",
            Self::GENERATE_REGION => "GenerateRegion",
            Self::GENVAR_DECLARATION => "GenvarDeclaration",
            Self::GREATER_THAN_EQUAL_EXPRESSION => "GreaterThanEqualExpression",
            Self::GREATER_THAN_EXPRESSION => "GreaterThanExpression",
            Self::HIERARCHICAL_INSTANCE => "HierarchicalInstance",
            Self::HIERARCHY_INSTANTIATION => "HierarchyInstantiation",
            Self::ID_WITH_EXPR_COVERAGE_BIN_INITIALIZER => "IdWithExprCoverageBinInitializer",
            Self::IDENTIFIER_NAME => "IdentifierName",
            Self::IDENTIFIER_SELECT_NAME => "IdentifierSelectName",
            Self::IF_DEF_DIRECTIVE => "IfDefDirective",
            Self::IF_GENERATE => "IfGenerate",
            Self::IF_N_DEF_DIRECTIVE => "IfNDefDirective",
            Self::IF_NONE_PATH_DECLARATION => "IfNonePathDeclaration",
            Self::IFF_EVENT_CLAUSE => "IffEventClause",
            Self::IFF_PROPERTY_EXPR => "IffPropertyExpr",
            Self::IMMEDIATE_ASSERT_STATEMENT => "ImmediateAssertStatement",
            Self::IMMEDIATE_ASSERTION_MEMBER => "ImmediateAssertionMember",
            Self::IMMEDIATE_ASSUME_STATEMENT => "ImmediateAssumeStatement",
            Self::IMMEDIATE_COVER_STATEMENT => "ImmediateCoverStatement",
            Self::IMPLEMENTS_CLAUSE => "ImplementsClause",
            Self::IMPLICATION_CONSTRAINT => "ImplicationConstraint",
            Self::IMPLICATION_PROPERTY_EXPR => "ImplicationPropertyExpr",
            Self::IMPLICIT_ANSI_PORT => "ImplicitAnsiPort",
            Self::IMPLICIT_EVENT_CONTROL => "ImplicitEventControl",
            Self::IMPLICIT_NON_ANSI_PORT => "ImplicitNonAnsiPort",
            Self::IMPLICIT_TYPE => "ImplicitType",
            Self::IMPLIES_PROPERTY_EXPR => "ImpliesPropertyExpr",
            Self::INCLUDE_DIRECTIVE => "IncludeDirective",
            Self::INEQUALITY_EXPRESSION => "InequalityExpression",
            Self::INITIAL_BLOCK => "InitialBlock",
            Self::INSIDE_EXPRESSION => "InsideExpression",
            Self::INSTANCE_CONFIG_RULE => "InstanceConfigRule",
            Self::INSTANCE_NAME => "InstanceName",
            Self::INT_TYPE => "IntType",
            Self::INTEGER_LITERAL_EXPRESSION => "IntegerLiteralExpression",
            Self::INTEGER_TYPE => "IntegerType",
            Self::INTEGER_VECTOR_EXPRESSION => "IntegerVectorExpression",
            Self::INTERFACE_DECLARATION => "InterfaceDeclaration",
            Self::INTERFACE_HEADER => "InterfaceHeader",
            Self::INTERFACE_PORT_HEADER => "InterfacePortHeader",
            Self::INTERSECT_CLAUSE => "IntersectClause",
            Self::INTERSECT_SEQUENCE_EXPR => "IntersectSequenceExpr",
            Self::INVOCATION_EXPRESSION => "InvocationExpression",
            Self::JUMP_STATEMENT => "JumpStatement",
            Self::LESS_THAN_EQUAL_EXPRESSION => "LessThanEqualExpression",
            Self::LESS_THAN_EXPRESSION => "LessThanExpression",
            Self::LET_DECLARATION => "LetDeclaration",
            Self::LIBRARY_DECLARATION => "LibraryDeclaration",
            Self::LIBRARY_INC_DIR_CLAUSE => "LibraryIncDirClause",
            Self::LIBRARY_INCLUDE_STATEMENT => "LibraryIncludeStatement",
            Self::LIBRARY_MAP => "LibraryMap",
            Self::LINE_DIRECTIVE => "LineDirective",
            Self::LOCAL_SCOPE => "LocalScope",
            Self::LOCAL_VARIABLE_DECLARATION => "LocalVariableDeclaration",
            Self::LOGIC_TYPE => "LogicType",
            Self::LOGICAL_AND_EXPRESSION => "LogicalAndExpression",
            Self::LOGICAL_EQUIVALENCE_EXPRESSION => "LogicalEquivalenceExpression",
            Self::LOGICAL_IMPLICATION_EXPRESSION => "LogicalImplicationExpression",
            Self::LOGICAL_LEFT_SHIFT_ASSIGNMENT_EXPRESSION => {
                "LogicalLeftShiftAssignmentExpression"
            }
            Self::LOGICAL_OR_EXPRESSION => "LogicalOrExpression",
            Self::LOGICAL_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION => {
                "LogicalRightShiftAssignmentExpression"
            }
            Self::LOGICAL_SHIFT_LEFT_EXPRESSION => "LogicalShiftLeftExpression",
            Self::LOGICAL_SHIFT_RIGHT_EXPRESSION => "LogicalShiftRightExpression",
            Self::LONG_INT_TYPE => "LongIntType",
            Self::LOOP_CONSTRAINT => "LoopConstraint",
            Self::LOOP_GENERATE => "LoopGenerate",
            Self::LOOP_STATEMENT => "LoopStatement",
            Self::MACRO_ACTUAL_ARGUMENT => "MacroActualArgument",
            Self::MACRO_ACTUAL_ARGUMENT_LIST => "MacroActualArgumentList",
            Self::MACRO_ARGUMENT_DEFAULT => "MacroArgumentDefault",
            Self::MACRO_FORMAL_ARGUMENT => "MacroFormalArgument",
            Self::MACRO_FORMAL_ARGUMENT_LIST => "MacroFormalArgumentList",
            Self::MACRO_USAGE => "MacroUsage",
            Self::MATCHES_CLAUSE => "MatchesClause",
            Self::MEMBER_ACCESS_EXPRESSION => "MemberAccessExpression",
            Self::MIN_TYP_MAX_EXPRESSION => "MinTypMaxExpression",
            Self::MOD_ASSIGNMENT_EXPRESSION => "ModAssignmentExpression",
            Self::MOD_EXPRESSION => "ModExpression",
            Self::MODPORT_CLOCKING_PORT => "ModportClockingPort",
            Self::MODPORT_DECLARATION => "ModportDeclaration",
            Self::MODPORT_EXPLICIT_PORT => "ModportExplicitPort",
            Self::MODPORT_ITEM => "ModportItem",
            Self::MODPORT_NAMED_PORT => "ModportNamedPort",
            Self::MODPORT_SIMPLE_PORT_LIST => "ModportSimplePortList",
            Self::MODPORT_SUBROUTINE_PORT => "ModportSubroutinePort",
            Self::MODPORT_SUBROUTINE_PORT_LIST => "ModportSubroutinePortList",
            Self::MODULE_DECLARATION => "ModuleDeclaration",
            Self::MODULE_HEADER => "ModuleHeader",
            Self::MULTIPLE_CONCATENATION_EXPRESSION => "MultipleConcatenationExpression",
            Self::MULTIPLY_ASSIGNMENT_EXPRESSION => "MultiplyAssignmentExpression",
            Self::MULTIPLY_EXPRESSION => "MultiplyExpression",
            Self::NAME_VALUE_PRAGMA_EXPRESSION => "NameValuePragmaExpression",
            Self::NAMED_ARGUMENT => "NamedArgument",
            Self::NAMED_BLOCK_CLAUSE => "NamedBlockClause",
            Self::NAMED_CONDITIONAL_DIRECTIVE_EXPRESSION => "NamedConditionalDirectiveExpression",
            Self::NAMED_LABEL => "NamedLabel",
            Self::NAMED_PARAM_ASSIGNMENT => "NamedParamAssignment",
            Self::NAMED_PORT_CONNECTION => "NamedPortConnection",
            Self::NAMED_STRUCTURE_PATTERN_MEMBER => "NamedStructurePatternMember",
            Self::NAMED_TYPE => "NamedType",
            Self::NET_ALIAS => "NetAlias",
            Self::NET_DECLARATION => "NetDeclaration",
            Self::NET_PORT_HEADER => "NetPortHeader",
            Self::NET_TYPE_DECLARATION => "NetTypeDeclaration",
            Self::NEW_ARRAY_EXPRESSION => "NewArrayExpression",
            Self::NEW_CLASS_EXPRESSION => "NewClassExpression",
            Self::NO_UNCONNECTED_DRIVE_DIRECTIVE => "NoUnconnectedDriveDirective",
            Self::NON_ANSI_PORT_LIST => "NonAnsiPortList",
            Self::NON_ANSI_UDP_PORT_LIST => "NonAnsiUdpPortList",
            Self::NONBLOCKING_ASSIGNMENT_EXPRESSION => "NonblockingAssignmentExpression",
            Self::NONBLOCKING_EVENT_TRIGGER_STATEMENT => "NonblockingEventTriggerStatement",
            Self::NULL_LITERAL_EXPRESSION => "NullLiteralExpression",
            Self::NUMBER_PRAGMA_EXPRESSION => "NumberPragmaExpression",
            Self::ONE_STEP_DELAY => "OneStepDelay",
            Self::OR_ASSIGNMENT_EXPRESSION => "OrAssignmentExpression",
            Self::OR_PROPERTY_EXPR => "OrPropertyExpr",
            Self::OR_SEQUENCE_EXPR => "OrSequenceExpr",
            Self::ORDERED_ARGUMENT => "OrderedArgument",
            Self::ORDERED_PARAM_ASSIGNMENT => "OrderedParamAssignment",
            Self::ORDERED_PORT_CONNECTION => "OrderedPortConnection",
            Self::ORDERED_STRUCTURE_PATTERN_MEMBER => "OrderedStructurePatternMember",
            Self::PACKAGE_DECLARATION => "PackageDeclaration",
            Self::PACKAGE_EXPORT_ALL_DECLARATION => "PackageExportAllDeclaration",
            Self::PACKAGE_EXPORT_DECLARATION => "PackageExportDeclaration",
            Self::PACKAGE_HEADER => "PackageHeader",
            Self::PACKAGE_IMPORT_DECLARATION => "PackageImportDeclaration",
            Self::PACKAGE_IMPORT_ITEM => "PackageImportItem",
            Self::PARALLEL_BLOCK_STATEMENT => "ParallelBlockStatement",
            Self::PARAMETER_DECLARATION => "ParameterDeclaration",
            Self::PARAMETER_DECLARATION_STATEMENT => "ParameterDeclarationStatement",
            Self::PARAMETER_PORT_LIST => "ParameterPortList",
            Self::PARAMETER_VALUE_ASSIGNMENT => "ParameterValueAssignment",
            Self::PAREN_EXPRESSION_LIST => "ParenExpressionList",
            Self::PAREN_PRAGMA_EXPRESSION => "ParenPragmaExpression",
            Self::PARENTHESIZED_BINS_SELECT_EXPR => "ParenthesizedBinsSelectExpr",
            Self::PARENTHESIZED_CONDITIONAL_DIRECTIVE_EXPRESSION => {
                "ParenthesizedConditionalDirectiveExpression"
            }
            Self::PARENTHESIZED_EVENT_EXPRESSION => "ParenthesizedEventExpression",
            Self::PARENTHESIZED_EXPRESSION => "ParenthesizedExpression",
            Self::PARENTHESIZED_PATTERN => "ParenthesizedPattern",
            Self::PARENTHESIZED_PROPERTY_EXPR => "ParenthesizedPropertyExpr",
            Self::PARENTHESIZED_SEQUENCE_EXPR => "ParenthesizedSequenceExpr",
            Self::PATH_DECLARATION => "PathDeclaration",
            Self::PATH_DESCRIPTION => "PathDescription",
            Self::PATTERN_CASE_ITEM => "PatternCaseItem",
            Self::PORT_CONCATENATION => "PortConcatenation",
            Self::PORT_DECLARATION => "PortDeclaration",
            Self::PORT_REFERENCE => "PortReference",
            Self::POSTDECREMENT_EXPRESSION => "PostdecrementExpression",
            Self::POSTINCREMENT_EXPRESSION => "PostincrementExpression",
            Self::POWER_EXPRESSION => "PowerExpression",
            Self::PRAGMA_DIRECTIVE => "PragmaDirective",
            Self::PRIMARY_BLOCK_EVENT_EXPRESSION => "PrimaryBlockEventExpression",
            Self::PRIMITIVE_INSTANTIATION => "PrimitiveInstantiation",
            Self::PROCEDURAL_ASSIGN_STATEMENT => "ProceduralAssignStatement",
            Self::PROCEDURAL_DEASSIGN_STATEMENT => "ProceduralDeassignStatement",
            Self::PROCEDURAL_FORCE_STATEMENT => "ProceduralForceStatement",
            Self::PROCEDURAL_RELEASE_STATEMENT => "ProceduralReleaseStatement",
            Self::PRODUCTION => "Production",
            Self::PROGRAM_DECLARATION => "ProgramDeclaration",
            Self::PROGRAM_HEADER => "ProgramHeader",
            Self::PROPERTY_DECLARATION => "PropertyDeclaration",
            Self::PROPERTY_SPEC => "PropertySpec",
            Self::PROPERTY_TYPE => "PropertyType",
            Self::PROTECT_DIRECTIVE => "ProtectDirective",
            Self::PROTECTED_DIRECTIVE => "ProtectedDirective",
            Self::PULL_STRENGTH => "PullStrength",
            Self::PULSE_STYLE_DECLARATION => "PulseStyleDeclaration",
            Self::QUEUE_DIMENSION_SPECIFIER => "QueueDimensionSpecifier",
            Self::RAND_CASE_ITEM => "RandCaseItem",
            Self::RAND_CASE_STATEMENT => "RandCaseStatement",
            Self::RAND_JOIN_CLAUSE => "RandJoinClause",
            Self::RAND_SEQUENCE_STATEMENT => "RandSequenceStatement",
            Self::RANGE_COVERAGE_BIN_INITIALIZER => "RangeCoverageBinInitializer",
            Self::RANGE_DIMENSION_SPECIFIER => "RangeDimensionSpecifier",
            Self::RANGE_LIST => "RangeList",
            Self::REAL_LITERAL_EXPRESSION => "RealLiteralExpression",
            Self::REAL_TIME_TYPE => "RealTimeType",
            Self::REAL_TYPE => "RealType",
            Self::REG_TYPE => "RegType",
            Self::REPEATED_EVENT_CONTROL => "RepeatedEventControl",
            Self::REPLICATED_ASSIGNMENT_PATTERN => "ReplicatedAssignmentPattern",
            Self::RESET_ALL_DIRECTIVE => "ResetAllDirective",
            Self::RESTRICT_PROPERTY_STATEMENT => "RestrictPropertyStatement",
            Self::RETURN_STATEMENT => "ReturnStatement",
            Self::ROOT_SCOPE => "RootScope",
            Self::RS_CASE => "RsCase",
            Self::RS_CODE_BLOCK => "RsCodeBlock",
            Self::RS_ELSE_CLAUSE => "RsElseClause",
            Self::RS_IF_ELSE => "RsIfElse",
            Self::RS_PROD_ITEM => "RsProdItem",
            Self::RS_REPEAT => "RsRepeat",
            Self::RS_RULE => "RsRule",
            Self::RS_WEIGHT_CLAUSE => "RsWeightClause",
            Self::S_UNTIL_PROPERTY_EXPR => "SUntilPropertyExpr",
            Self::S_UNTIL_WITH_PROPERTY_EXPR => "SUntilWithPropertyExpr",
            Self::SCOPED_NAME => "ScopedName",
            Self::SEQUENCE_DECLARATION => "SequenceDeclaration",
            Self::SEQUENCE_MATCH_LIST => "SequenceMatchList",
            Self::SEQUENCE_REPETITION => "SequenceRepetition",
            Self::SEQUENCE_TYPE => "SequenceType",
            Self::SEQUENTIAL_BLOCK_STATEMENT => "SequentialBlockStatement",
            Self::SHORT_INT_TYPE => "ShortIntType",
            Self::SHORT_REAL_TYPE => "ShortRealType",
            Self::SIGNAL_EVENT_EXPRESSION => "SignalEventExpression",
            Self::SIGNED_CAST_EXPRESSION => "SignedCastExpression",
            Self::SIMPLE_ASSIGNMENT_PATTERN => "SimpleAssignmentPattern",
            Self::SIMPLE_BINS_SELECT_EXPR => "SimpleBinsSelectExpr",
            Self::SIMPLE_PATH_SUFFIX => "SimplePathSuffix",
            Self::SIMPLE_PRAGMA_EXPRESSION => "SimplePragmaExpression",
            Self::SIMPLE_PROPERTY_EXPR => "SimplePropertyExpr",
            Self::SIMPLE_RANGE_SELECT => "SimpleRangeSelect",
            Self::SIMPLE_SEQUENCE_EXPR => "SimpleSequenceExpr",
            Self::SOLVE_BEFORE_CONSTRAINT => "SolveBeforeConstraint",
            Self::SPECIFY_BLOCK => "SpecifyBlock",
            Self::SPECPARAM_DECLARATION => "SpecparamDeclaration",
            Self::SPECPARAM_DECLARATOR => "SpecparamDeclarator",
            Self::STANDARD_CASE_ITEM => "StandardCaseItem",
            Self::STANDARD_PROPERTY_CASE_ITEM => "StandardPropertyCaseItem",
            Self::STANDARD_RS_CASE_ITEM => "StandardRsCaseItem",
            Self::STREAM_EXPRESSION => "StreamExpression",
            Self::STREAM_EXPRESSION_WITH_RANGE => "StreamExpressionWithRange",
            Self::STREAMING_CONCATENATION_EXPRESSION => "StreamingConcatenationExpression",
            Self::STRING_LITERAL_EXPRESSION => "StringLiteralExpression",
            Self::STRING_TYPE => "StringType",
            Self::STRONG_WEAK_PROPERTY_EXPR => "StrongWeakPropertyExpr",
            Self::STRUCT_TYPE => "StructType",
            Self::STRUCT_UNION_MEMBER => "StructUnionMember",
            Self::STRUCTURE_PATTERN => "StructurePattern",
            Self::STRUCTURED_ASSIGNMENT_PATTERN => "StructuredAssignmentPattern",
            Self::SUBTRACT_ASSIGNMENT_EXPRESSION => "SubtractAssignmentExpression",
            Self::SUBTRACT_EXPRESSION => "SubtractExpression",
            Self::SUPER_HANDLE => "SuperHandle",
            Self::SUPER_NEW_DEFAULTED_ARGS_EXPRESSION => "SuperNewDefaultedArgsExpression",
            Self::SYSTEM_NAME => "SystemName",
            Self::SYSTEM_TIMING_CHECK => "SystemTimingCheck",
            Self::TAGGED_PATTERN => "TaggedPattern",
            Self::TAGGED_UNION_EXPRESSION => "TaggedUnionExpression",
            Self::TASK_DECLARATION => "TaskDeclaration",
            Self::THIS_HANDLE => "ThisHandle",
            Self::THROUGHOUT_SEQUENCE_EXPR => "ThroughoutSequenceExpr",
            Self::TIME_LITERAL_EXPRESSION => "TimeLiteralExpression",
            Self::TIME_SCALE_DIRECTIVE => "TimeScaleDirective",
            Self::TIME_TYPE => "TimeType",
            Self::TIME_UNITS_DECLARATION => "TimeUnitsDeclaration",
            Self::TIMING_CHECK_EVENT_ARG => "TimingCheckEventArg",
            Self::TIMING_CHECK_EVENT_CONDITION => "TimingCheckEventCondition",
            Self::TIMING_CONTROL_EXPRESSION => "TimingControlExpression",
            Self::TIMING_CONTROL_STATEMENT => "TimingControlStatement",
            Self::TRANS_LIST_COVERAGE_BIN_INITIALIZER => "TransListCoverageBinInitializer",
            Self::TRANS_RANGE => "TransRange",
            Self::TRANS_REPEAT_RANGE => "TransRepeatRange",
            Self::TRANS_SET => "TransSet",
            Self::TYPE_ASSIGNMENT => "TypeAssignment",
            Self::TYPE_PARAMETER_DECLARATION => "TypeParameterDeclaration",
            Self::TYPE_REFERENCE => "TypeReference",
            Self::TYPEDEF_DECLARATION => "TypedefDeclaration",
            Self::UDP_BODY => "UdpBody",
            Self::UDP_DECLARATION => "UdpDeclaration",
            Self::UDP_EDGE_FIELD => "UdpEdgeField",
            Self::UDP_ENTRY => "UdpEntry",
            Self::UDP_INITIAL_STMT => "UdpInitialStmt",
            Self::UDP_INPUT_PORT_DECL => "UdpInputPortDecl",
            Self::UDP_OUTPUT_PORT_DECL => "UdpOutputPortDecl",
            Self::UDP_SIMPLE_FIELD => "UdpSimpleField",
            Self::UNARY_BINS_SELECT_EXPR => "UnaryBinsSelectExpr",
            Self::UNARY_BITWISE_AND_EXPRESSION => "UnaryBitwiseAndExpression",
            Self::UNARY_BITWISE_NAND_EXPRESSION => "UnaryBitwiseNandExpression",
            Self::UNARY_BITWISE_NOR_EXPRESSION => "UnaryBitwiseNorExpression",
            Self::UNARY_BITWISE_NOT_EXPRESSION => "UnaryBitwiseNotExpression",
            Self::UNARY_BITWISE_OR_EXPRESSION => "UnaryBitwiseOrExpression",
            Self::UNARY_BITWISE_XNOR_EXPRESSION => "UnaryBitwiseXnorExpression",
            Self::UNARY_BITWISE_XOR_EXPRESSION => "UnaryBitwiseXorExpression",
            Self::UNARY_CONDITIONAL_DIRECTIVE_EXPRESSION => "UnaryConditionalDirectiveExpression",
            Self::UNARY_LOGICAL_NOT_EXPRESSION => "UnaryLogicalNotExpression",
            Self::UNARY_MINUS_EXPRESSION => "UnaryMinusExpression",
            Self::UNARY_PLUS_EXPRESSION => "UnaryPlusExpression",
            Self::UNARY_PREDECREMENT_EXPRESSION => "UnaryPredecrementExpression",
            Self::UNARY_PREINCREMENT_EXPRESSION => "UnaryPreincrementExpression",
            Self::UNARY_PROPERTY_EXPR => "UnaryPropertyExpr",
            Self::UNARY_SELECT_PROPERTY_EXPR => "UnarySelectPropertyExpr",
            Self::UNBASED_UNSIZED_LITERAL_EXPRESSION => "UnbasedUnsizedLiteralExpression",
            Self::UNCONNECTED_DRIVE_DIRECTIVE => "UnconnectedDriveDirective",
            Self::UNDEF_DIRECTIVE => "UndefDirective",
            Self::UNDEFINE_ALL_DIRECTIVE => "UndefineAllDirective",
            Self::UNION_TYPE => "UnionType",
            Self::UNIQUENESS_CONSTRAINT => "UniquenessConstraint",
            Self::UNIT_SCOPE => "UnitScope",
            Self::UNTIL_PROPERTY_EXPR => "UntilPropertyExpr",
            Self::UNTIL_WITH_PROPERTY_EXPR => "UntilWithPropertyExpr",
            Self::UNTYPED => "Untyped",
            Self::USER_DEFINED_NET_DECLARATION => "UserDefinedNetDeclaration",
            Self::VALUE_RANGE_EXPRESSION => "ValueRangeExpression",
            Self::VARIABLE_DIMENSION => "VariableDimension",
            Self::VARIABLE_PATTERN => "VariablePattern",
            Self::VARIABLE_PORT_HEADER => "VariablePortHeader",
            Self::VIRTUAL_INTERFACE_TYPE => "VirtualInterfaceType",
            Self::VOID_CASTED_CALL_STATEMENT => "VoidCastedCallStatement",
            Self::VOID_TYPE => "VoidType",
            Self::WAIT_FORK_STATEMENT => "WaitForkStatement",
            Self::WAIT_ORDER_STATEMENT => "WaitOrderStatement",
            Self::WAIT_STATEMENT => "WaitStatement",
            Self::WILDCARD_DIMENSION_SPECIFIER => "WildcardDimensionSpecifier",
            Self::WILDCARD_EQUALITY_EXPRESSION => "WildcardEqualityExpression",
            Self::WILDCARD_INEQUALITY_EXPRESSION => "WildcardInequalityExpression",
            Self::WILDCARD_LITERAL_EXPRESSION => "WildcardLiteralExpression",
            Self::WILDCARD_PATTERN => "WildcardPattern",
            Self::WILDCARD_PORT_CONNECTION => "WildcardPortConnection",
            Self::WILDCARD_PORT_LIST => "WildcardPortList",
            Self::WILDCARD_UDP_PORT_LIST => "WildcardUdpPortList",
            Self::WITH_CLAUSE => "WithClause",
            Self::WITH_FUNCTION_CLAUSE => "WithFunctionClause",
            Self::WITH_FUNCTION_SAMPLE => "WithFunctionSample",
            Self::WITHIN_SEQUENCE_EXPR => "WithinSequenceExpr",
            Self::XOR_ASSIGNMENT_EXPRESSION => "XorAssignmentExpression",
            _ => unreachable!(),
        };
        f.write_str(name)
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TokenKind(u16);
impl TokenKind {
    pub const ACCEPT_ON_KEYWORD: Self = Self(93u16);
    pub const ALIAS_KEYWORD: Self = Self(94u16);
    pub const ALWAYS_COMB_KEYWORD: Self = Self(96u16);
    pub const ALWAYS_FF_KEYWORD: Self = Self(97u16);
    pub const ALWAYS_KEYWORD: Self = Self(95u16);
    pub const ALWAYS_LATCH_KEYWORD: Self = Self(98u16);
    pub const AND: Self = Self(89u16);
    pub const AND_EQUAL: Self = Self(61u16);
    pub const AND_KEYWORD: Self = Self(99u16);
    pub const APOSTROPHE: Self = Self(11u16);
    pub const APOSTROPHE_OPEN_BRACE: Self = Self(12u16);
    pub const ASSERT_KEYWORD: Self = Self(100u16);
    pub const ASSIGN_KEYWORD: Self = Self(101u16);
    pub const ASSUME_KEYWORD: Self = Self(102u16);
    pub const AT: Self = Self(87u16);
    pub const AUTOMATIC_KEYWORD: Self = Self(103u16);
    pub const BEFORE_KEYWORD: Self = Self(104u16);
    pub const BEGIN_KEYWORD: Self = Self(105u16);
    pub const BIND_KEYWORD: Self = Self(106u16);
    pub const BINS_KEYWORD: Self = Self(107u16);
    pub const BINS_OF_KEYWORD: Self = Self(108u16);
    pub const BIT_KEYWORD: Self = Self(109u16);
    pub const BREAK_KEYWORD: Self = Self(110u16);
    pub const BUF_IF_0_KEYWORD: Self = Self(112u16);
    pub const BUF_IF_1_KEYWORD: Self = Self(113u16);
    pub const BUF_KEYWORD: Self = Self(111u16);
    pub const BYTE_KEYWORD: Self = Self(114u16);
    pub const CASE_KEYWORD: Self = Self(115u16);
    pub const CASE_X_KEYWORD: Self = Self(116u16);
    pub const CASE_Z_KEYWORD: Self = Self(117u16);
    pub const CELL_KEYWORD: Self = Self(118u16);
    pub const CHECKER_KEYWORD: Self = Self(120u16);
    pub const CLASS_KEYWORD: Self = Self(121u16);
    pub const CLOCKING_KEYWORD: Self = Self(122u16);
    pub const CLOSE_BRACE: Self = Self(14u16);
    pub const CLOSE_BRACKET: Self = Self(16u16);
    pub const CLOSE_PARENTHESIS: Self = Self(18u16);
    pub const CMOS_KEYWORD: Self = Self(123u16);
    pub const COLON: Self = Self(20u16);
    pub const COLON_EQUALS: Self = Self(21u16);
    pub const COLON_SLASH: Self = Self(22u16);
    pub const COMMA: Self = Self(24u16);
    pub const CONFIG_KEYWORD: Self = Self(124u16);
    pub const CONSTRAINT_KEYWORD: Self = Self(126u16);
    pub const CONST_KEYWORD: Self = Self(125u16);
    pub const CONTEXT_KEYWORD: Self = Self(127u16);
    pub const CONTINUE_KEYWORD: Self = Self(128u16);
    pub const COVER_GROUP_KEYWORD: Self = Self(130u16);
    pub const COVER_KEYWORD: Self = Self(129u16);
    pub const COVER_POINT_KEYWORD: Self = Self(131u16);
    pub const CROSS_KEYWORD: Self = Self(132u16);
    pub const C_HANDLE_KEYWORD: Self = Self(119u16);
    pub const DEASSIGN_KEYWORD: Self = Self(133u16);
    pub const DEFAULT_KEYWORD: Self = Self(134u16);
    pub const DEF_PARAM_KEYWORD: Self = Self(135u16);
    pub const DESIGN_KEYWORD: Self = Self(136u16);
    pub const DIRECTIVE: Self = Self(343u16);
    pub const DISABLE_KEYWORD: Self = Self(137u16);
    pub const DIST_KEYWORD: Self = Self(138u16);
    pub const DOLLAR: Self = Self(44u16);
    pub const DOT: Self = Self(25u16);
    pub const DOUBLE_AND: Self = Self(90u16);
    pub const DOUBLE_AT: Self = Self(88u16);
    pub const DOUBLE_COLON: Self = Self(23u16);
    pub const DOUBLE_EQUALS: Self = Self(53u16);
    pub const DOUBLE_EQUALS_QUESTION: Self = Self(54u16);
    pub const DOUBLE_HASH: Self = Self(47u16);
    pub const DOUBLE_MINUS: Self = Self(36u16);
    pub const DOUBLE_OR: Self = Self(84u16);
    pub const DOUBLE_PLUS: Self = Self(31u16);
    pub const DOUBLE_STAR: Self = Self(28u16);
    pub const DO_KEYWORD: Self = Self(139u16);
    pub const EDGE_KEYWORD: Self = Self(140u16);
    pub const ELSE_KEYWORD: Self = Self(141u16);
    pub const EMPTY_MACRO_ARGUMENT: Self = Self(350u16);
    pub const END_CASE_KEYWORD: Self = Self(143u16);
    pub const END_CHECKER_KEYWORD: Self = Self(144u16);
    pub const END_CLASS_KEYWORD: Self = Self(145u16);
    pub const END_CLOCKING_KEYWORD: Self = Self(146u16);
    pub const END_CONFIG_KEYWORD: Self = Self(147u16);
    pub const END_FUNCTION_KEYWORD: Self = Self(148u16);
    pub const END_GENERATE_KEYWORD: Self = Self(149u16);
    pub const END_GROUP_KEYWORD: Self = Self(150u16);
    pub const END_INTERFACE_KEYWORD: Self = Self(151u16);
    pub const END_KEYWORD: Self = Self(142u16);
    pub const END_MODULE_KEYWORD: Self = Self(152u16);
    pub const END_OF_FILE: Self = Self(1u16);
    pub const END_PACKAGE_KEYWORD: Self = Self(153u16);
    pub const END_PRIMITIVE_KEYWORD: Self = Self(154u16);
    pub const END_PROGRAM_KEYWORD: Self = Self(155u16);
    pub const END_PROPERTY_KEYWORD: Self = Self(156u16);
    pub const END_SEQUENCE_KEYWORD: Self = Self(158u16);
    pub const END_SPECIFY_KEYWORD: Self = Self(157u16);
    pub const END_TABLE_KEYWORD: Self = Self(159u16);
    pub const END_TASK_KEYWORD: Self = Self(160u16);
    pub const ENUM_KEYWORD: Self = Self(161u16);
    pub const EQUALS: Self = Self(52u16);
    pub const EQUALS_ARROW: Self = Self(56u16);
    pub const EVENTUALLY_KEYWORD: Self = Self(163u16);
    pub const EVENT_KEYWORD: Self = Self(162u16);
    pub const EXCLAMATION: Self = Self(73u16);
    pub const EXCLAMATION_DOUBLE_EQUALS: Self = Self(76u16);
    pub const EXCLAMATION_EQUALS: Self = Self(74u16);
    pub const EXCLAMATION_EQUALS_QUESTION: Self = Self(75u16);
    pub const EXPECT_KEYWORD: Self = Self(164u16);
    pub const EXPORT_KEYWORD: Self = Self(165u16);
    pub const EXTENDS_KEYWORD: Self = Self(166u16);
    pub const EXTERN_KEYWORD: Self = Self(167u16);
    pub const FINAL_KEYWORD: Self = Self(168u16);
    pub const FIRST_MATCH_KEYWORD: Self = Self(169u16);
    pub const FORCE_KEYWORD: Self = Self(171u16);
    pub const FOREACH_KEYWORD: Self = Self(172u16);
    pub const FOREVER_KEYWORD: Self = Self(173u16);
    pub const FORK_JOIN_KEYWORD: Self = Self(175u16);
    pub const FORK_KEYWORD: Self = Self(174u16);
    pub const FOR_KEYWORD: Self = Self(170u16);
    pub const FUNCTION_KEYWORD: Self = Self(176u16);
    pub const GENERATE_KEYWORD: Self = Self(177u16);
    pub const GEN_VAR_KEYWORD: Self = Self(178u16);
    pub const GLOBAL_KEYWORD: Self = Self(179u16);
    pub const GREATER_THAN: Self = Self(81u16);
    pub const GREATER_THAN_EQUALS: Self = Self(82u16);
    pub const HASH: Self = Self(46u16);
    pub const HASH_EQUALS_HASH: Self = Self(49u16);
    pub const HASH_MINUS_HASH: Self = Self(48u16);
    pub const HIGH_Z0_KEYWORD: Self = Self(180u16);
    pub const HIGH_Z1_KEYWORD: Self = Self(181u16);
    pub const IDENTIFIER: Self = Self(2u16);
    pub const IFF_KEYWORD: Self = Self(183u16);
    pub const IF_KEYWORD: Self = Self(182u16);
    pub const IF_NONE_KEYWORD: Self = Self(184u16);
    pub const IGNORE_BINS_KEYWORD: Self = Self(185u16);
    pub const ILLEGAL_BINS_KEYWORD: Self = Self(186u16);
    pub const IMPLEMENTS_KEYWORD: Self = Self(187u16);
    pub const IMPLIES_KEYWORD: Self = Self(188u16);
    pub const IMPORT_KEYWORD: Self = Self(189u16);
    pub const INCLUDE_FILE_NAME: Self = Self(344u16);
    pub const INCLUDE_KEYWORD: Self = Self(191u16);
    pub const INC_DIR_KEYWORD: Self = Self(190u16);
    pub const INITIAL_KEYWORD: Self = Self(192u16);
    pub const INPUT_KEYWORD: Self = Self(194u16);
    pub const INSIDE_KEYWORD: Self = Self(195u16);
    pub const INSTANCE_KEYWORD: Self = Self(196u16);
    pub const INTEGER_BASE: Self = Self(6u16);
    pub const INTEGER_KEYWORD: Self = Self(198u16);
    pub const INTEGER_LITERAL: Self = Self(5u16);
    pub const INTERCONNECT_KEYWORD: Self = Self(199u16);
    pub const INTERFACE_KEYWORD: Self = Self(200u16);
    pub const INTERSECT_KEYWORD: Self = Self(201u16);
    pub const INT_KEYWORD: Self = Self(197u16);
    pub const IN_OUT_KEYWORD: Self = Self(193u16);
    pub const JOIN_ANY_KEYWORD: Self = Self(203u16);
    pub const JOIN_KEYWORD: Self = Self(202u16);
    pub const JOIN_NONE_KEYWORD: Self = Self(204u16);
    pub const LARGE_KEYWORD: Self = Self(205u16);
    pub const LEFT_SHIFT: Self = Self(69u16);
    pub const LEFT_SHIFT_EQUAL: Self = Self(65u16);
    pub const LESS_THAN: Self = Self(78u16);
    pub const LESS_THAN_EQUALS: Self = Self(79u16);
    pub const LESS_THAN_MINUS_ARROW: Self = Self(80u16);
    pub const LET_KEYWORD: Self = Self(206u16);
    pub const LIBRARY_KEYWORD: Self = Self(208u16);
    pub const LIB_LIST_KEYWORD: Self = Self(207u16);
    pub const LINE_CONTINUATION: Self = Self(351u16);
    pub const LOCAL_KEYWORD: Self = Self(209u16);
    pub const LOCAL_PARAM_KEYWORD: Self = Self(210u16);
    pub const LOGIC_KEYWORD: Self = Self(211u16);
    pub const LONG_INT_KEYWORD: Self = Self(212u16);
    pub const MACROMODULE_KEYWORD: Self = Self(213u16);
    pub const MACRO_ESCAPED_QUOTE: Self = Self(348u16);
    pub const MACRO_PASTE: Self = Self(349u16);
    pub const MACRO_QUOTE: Self = Self(346u16);
    pub const MACRO_TRIPLE_QUOTE: Self = Self(347u16);
    pub const MACRO_USAGE: Self = Self(345u16);
    pub const MATCHES_KEYWORD: Self = Self(214u16);
    pub const MEDIUM_KEYWORD: Self = Self(215u16);
    pub const MINUS: Self = Self(35u16);
    pub const MINUS_ARROW: Self = Self(38u16);
    pub const MINUS_COLON: Self = Self(37u16);
    pub const MINUS_DOUBLE_ARROW: Self = Self(39u16);
    pub const MINUS_EQUAL: Self = Self(58u16);
    pub const MODULE_KEYWORD: Self = Self(217u16);
    pub const MOD_PORT_KEYWORD: Self = Self(216u16);
    pub const NAND_KEYWORD: Self = Self(218u16);
    pub const NEG_EDGE_KEYWORD: Self = Self(219u16);
    pub const NET_TYPE_KEYWORD: Self = Self(220u16);
    pub const NEW_KEYWORD: Self = Self(221u16);
    pub const NEXT_TIME_KEYWORD: Self = Self(222u16);
    pub const NMOS_KEYWORD: Self = Self(223u16);
    pub const NOR_KEYWORD: Self = Self(224u16);
    pub const NOT_IF_0_KEYWORD: Self = Self(227u16);
    pub const NOT_IF_1_KEYWORD: Self = Self(228u16);
    pub const NOT_KEYWORD: Self = Self(226u16);
    pub const NO_SHOW_CANCELLED_KEYWORD: Self = Self(225u16);
    pub const NULL_KEYWORD: Self = Self(229u16);
    pub const ONE_STEP: Self = Self(92u16);
    pub const OPEN_BRACE: Self = Self(13u16);
    pub const OPEN_BRACKET: Self = Self(15u16);
    pub const OPEN_PARENTHESIS: Self = Self(17u16);
    pub const OR: Self = Self(83u16);
    pub const OR_EQUAL: Self = Self(62u16);
    pub const OR_EQUALS_ARROW: Self = Self(86u16);
    pub const OR_KEYWORD: Self = Self(230u16);
    pub const OR_MINUS_ARROW: Self = Self(85u16);
    pub const OUTPUT_KEYWORD: Self = Self(231u16);
    pub const PACKAGE_KEYWORD: Self = Self(232u16);
    pub const PACKED_KEYWORD: Self = Self(233u16);
    pub const PARAMETER_KEYWORD: Self = Self(234u16);
    pub const PERCENT: Self = Self(77u16);
    pub const PERCENT_EQUAL: Self = Self(63u16);
    pub const PLACEHOLDER: Self = Self(10u16);
    pub const PLUS: Self = Self(30u16);
    pub const PLUS_COLON: Self = Self(32u16);
    pub const PLUS_DIV_MINUS: Self = Self(33u16);
    pub const PLUS_EQUAL: Self = Self(57u16);
    pub const PLUS_MOD_MINUS: Self = Self(34u16);
    pub const PMOS_KEYWORD: Self = Self(235u16);
    pub const POS_EDGE_KEYWORD: Self = Self(236u16);
    pub const PRIMITIVE_KEYWORD: Self = Self(237u16);
    pub const PRIORITY_KEYWORD: Self = Self(238u16);
    pub const PROGRAM_KEYWORD: Self = Self(239u16);
    pub const PROPERTY_KEYWORD: Self = Self(240u16);
    pub const PROTECTED_KEYWORD: Self = Self(241u16);
    pub const PULL_0_KEYWORD: Self = Self(242u16);
    pub const PULL_1_KEYWORD: Self = Self(243u16);
    pub const PULL_DOWN_KEYWORD: Self = Self(244u16);
    pub const PULL_UP_KEYWORD: Self = Self(245u16);
    pub const PULSE_STYLE_ON_DETECT_KEYWORD: Self = Self(246u16);
    pub const PULSE_STYLE_ON_EVENT_KEYWORD: Self = Self(247u16);
    pub const PURE_KEYWORD: Self = Self(248u16);
    pub const QUESTION: Self = Self(45u16);
    pub const RAND_CASE_KEYWORD: Self = Self(251u16);
    pub const RAND_C_KEYWORD: Self = Self(250u16);
    pub const RAND_KEYWORD: Self = Self(249u16);
    pub const RAND_SEQUENCE_KEYWORD: Self = Self(252u16);
    pub const RCMOS_KEYWORD: Self = Self(253u16);
    pub const REAL_KEYWORD: Self = Self(254u16);
    pub const REAL_LITERAL: Self = Self(8u16);
    pub const REAL_TIME_KEYWORD: Self = Self(255u16);
    pub const REF_KEYWORD: Self = Self(256u16);
    pub const REG_KEYWORD: Self = Self(257u16);
    pub const REJECT_ON_KEYWORD: Self = Self(258u16);
    pub const RELEASE_KEYWORD: Self = Self(259u16);
    pub const REPEAT_KEYWORD: Self = Self(260u16);
    pub const RESTRICT_KEYWORD: Self = Self(261u16);
    pub const RETURN_KEYWORD: Self = Self(262u16);
    pub const RIGHT_SHIFT: Self = Self(70u16);
    pub const RIGHT_SHIFT_EQUAL: Self = Self(67u16);
    pub const RNMOS_KEYWORD: Self = Self(263u16);
    pub const ROOT_SYSTEM_NAME: Self = Self(342u16);
    pub const RPMOS_KEYWORD: Self = Self(264u16);
    pub const RTRAN_IF_0_KEYWORD: Self = Self(266u16);
    pub const RTRAN_IF_1_KEYWORD: Self = Self(267u16);
    pub const RTRAN_KEYWORD: Self = Self(265u16);
    pub const SCALARED_KEYWORD: Self = Self(273u16);
    pub const SEMICOLON: Self = Self(19u16);
    pub const SEQUENCE_KEYWORD: Self = Self(274u16);
    pub const SHORT_INT_KEYWORD: Self = Self(275u16);
    pub const SHORT_REAL_KEYWORD: Self = Self(276u16);
    pub const SHOW_CANCELLED_KEYWORD: Self = Self(277u16);
    pub const SIGNED_KEYWORD: Self = Self(278u16);
    pub const SLASH: Self = Self(26u16);
    pub const SLASH_EQUAL: Self = Self(59u16);
    pub const SMALL_KEYWORD: Self = Self(279u16);
    pub const SOFT_KEYWORD: Self = Self(280u16);
    pub const SOLVE_KEYWORD: Self = Self(281u16);
    pub const SPECIFY_KEYWORD: Self = Self(282u16);
    pub const SPEC_PARAM_KEYWORD: Self = Self(283u16);
    pub const STAR: Self = Self(27u16);
    pub const STAR_ARROW: Self = Self(29u16);
    pub const STAR_EQUAL: Self = Self(60u16);
    pub const STATIC_KEYWORD: Self = Self(284u16);
    pub const STRING_KEYWORD: Self = Self(285u16);
    pub const STRING_LITERAL: Self = Self(4u16);
    pub const STRONG_0_KEYWORD: Self = Self(287u16);
    pub const STRONG_1_KEYWORD: Self = Self(288u16);
    pub const STRONG_KEYWORD: Self = Self(286u16);
    pub const STRUCT_KEYWORD: Self = Self(289u16);
    pub const SUPER_KEYWORD: Self = Self(290u16);
    pub const SUPPLY_0_KEYWORD: Self = Self(291u16);
    pub const SUPPLY_1_KEYWORD: Self = Self(292u16);
    pub const SYNC_ACCEPT_ON_KEYWORD: Self = Self(293u16);
    pub const SYNC_REJECT_ON_KEYWORD: Self = Self(294u16);
    pub const SYSTEM_IDENTIFIER: Self = Self(3u16);
    pub const S_ALWAYS_KEYWORD: Self = Self(268u16);
    pub const S_EVENTUALLY_KEYWORD: Self = Self(269u16);
    pub const S_NEXT_TIME_KEYWORD: Self = Self(270u16);
    pub const S_UNTIL_KEYWORD: Self = Self(271u16);
    pub const S_UNTIL_WITH_KEYWORD: Self = Self(272u16);
    pub const TABLE_KEYWORD: Self = Self(295u16);
    pub const TAGGED_KEYWORD: Self = Self(296u16);
    pub const TASK_KEYWORD: Self = Self(297u16);
    pub const THIS_KEYWORD: Self = Self(298u16);
    pub const THROUGHOUT_KEYWORD: Self = Self(299u16);
    pub const TILDE: Self = Self(40u16);
    pub const TILDE_AND: Self = Self(41u16);
    pub const TILDE_OR: Self = Self(42u16);
    pub const TILDE_XOR: Self = Self(43u16);
    pub const TIME_KEYWORD: Self = Self(300u16);
    pub const TIME_LITERAL: Self = Self(9u16);
    pub const TIME_PRECISION_KEYWORD: Self = Self(301u16);
    pub const TIME_UNIT_KEYWORD: Self = Self(302u16);
    pub const TRAN_IF_0_KEYWORD: Self = Self(304u16);
    pub const TRAN_IF_1_KEYWORD: Self = Self(305u16);
    pub const TRAN_KEYWORD: Self = Self(303u16);
    pub const TRIPLE_AND: Self = Self(91u16);
    pub const TRIPLE_EQUALS: Self = Self(55u16);
    pub const TRIPLE_LEFT_SHIFT: Self = Self(71u16);
    pub const TRIPLE_LEFT_SHIFT_EQUAL: Self = Self(66u16);
    pub const TRIPLE_RIGHT_SHIFT: Self = Self(72u16);
    pub const TRIPLE_RIGHT_SHIFT_EQUAL: Self = Self(68u16);
    pub const TRI_0_KEYWORD: Self = Self(307u16);
    pub const TRI_1_KEYWORD: Self = Self(308u16);
    pub const TRI_AND_KEYWORD: Self = Self(309u16);
    pub const TRI_KEYWORD: Self = Self(306u16);
    pub const TRI_OR_KEYWORD: Self = Self(310u16);
    pub const TRI_REG_KEYWORD: Self = Self(311u16);
    pub const TYPEDEF_KEYWORD: Self = Self(313u16);
    pub const TYPE_KEYWORD: Self = Self(312u16);
    pub const UNBASED_UNSIZED_LITERAL: Self = Self(7u16);
    pub const UNION_KEYWORD: Self = Self(314u16);
    pub const UNIQUE_0_KEYWORD: Self = Self(316u16);
    pub const UNIQUE_KEYWORD: Self = Self(315u16);
    pub const UNIT_SYSTEM_NAME: Self = Self(341u16);
    pub const UNKNOWN: Self = Self(0u16);
    pub const UNSIGNED_KEYWORD: Self = Self(317u16);
    pub const UNTIL_KEYWORD: Self = Self(318u16);
    pub const UNTIL_WITH_KEYWORD: Self = Self(319u16);
    pub const UNTYPED_KEYWORD: Self = Self(320u16);
    pub const USE_KEYWORD: Self = Self(321u16);
    pub const U_WIRE_KEYWORD: Self = Self(322u16);
    pub const VAR_KEYWORD: Self = Self(323u16);
    pub const VECTORED_KEYWORD: Self = Self(324u16);
    pub const VIRTUAL_KEYWORD: Self = Self(325u16);
    pub const VOID_KEYWORD: Self = Self(326u16);
    pub const WAIT_KEYWORD: Self = Self(327u16);
    pub const WAIT_ORDER_KEYWORD: Self = Self(328u16);
    pub const WEAK_0_KEYWORD: Self = Self(331u16);
    pub const WEAK_1_KEYWORD: Self = Self(332u16);
    pub const WEAK_KEYWORD: Self = Self(330u16);
    pub const WHILE_KEYWORD: Self = Self(333u16);
    pub const WILDCARD_KEYWORD: Self = Self(334u16);
    pub const WIRE_KEYWORD: Self = Self(335u16);
    pub const WITHIN_KEYWORD: Self = Self(337u16);
    pub const WITH_KEYWORD: Self = Self(336u16);
    pub const W_AND_KEYWORD: Self = Self(329u16);
    pub const W_OR_KEYWORD: Self = Self(338u16);
    pub const XNOR_KEYWORD: Self = Self(339u16);
    pub const XOR: Self = Self(50u16);
    pub const XOR_EQUAL: Self = Self(64u16);
    pub const XOR_KEYWORD: Self = Self(340u16);
    pub const XOR_TILDE: Self = Self(51u16);

    pub fn from_id(id: u16) -> Self {
        Self(id)
    }
}
impl fmt::Debug for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match *self {
            Self::UNKNOWN => "Unknown",
            Self::END_OF_FILE => "EndOfFile",
            Self::IDENTIFIER => "Identifier",
            Self::SYSTEM_IDENTIFIER => "SystemIdentifier",
            Self::STRING_LITERAL => "StringLiteral",
            Self::INTEGER_LITERAL => "IntegerLiteral",
            Self::INTEGER_BASE => "IntegerBase",
            Self::UNBASED_UNSIZED_LITERAL => "UnbasedUnsizedLiteral",
            Self::REAL_LITERAL => "RealLiteral",
            Self::TIME_LITERAL => "TimeLiteral",
            Self::PLACEHOLDER => "Placeholder",
            Self::APOSTROPHE => "Apostrophe",
            Self::APOSTROPHE_OPEN_BRACE => "ApostropheOpenBrace",
            Self::OPEN_BRACE => "OpenBrace",
            Self::CLOSE_BRACE => "CloseBrace",
            Self::OPEN_BRACKET => "OpenBracket",
            Self::CLOSE_BRACKET => "CloseBracket",
            Self::OPEN_PARENTHESIS => "OpenParenthesis",
            Self::CLOSE_PARENTHESIS => "CloseParenthesis",
            Self::SEMICOLON => "Semicolon",
            Self::COLON => "Colon",
            Self::COLON_EQUALS => "ColonEquals",
            Self::COLON_SLASH => "ColonSlash",
            Self::DOUBLE_COLON => "DoubleColon",
            Self::COMMA => "Comma",
            Self::DOT => "Dot",
            Self::SLASH => "Slash",
            Self::STAR => "Star",
            Self::DOUBLE_STAR => "DoubleStar",
            Self::STAR_ARROW => "StarArrow",
            Self::PLUS => "Plus",
            Self::DOUBLE_PLUS => "DoublePlus",
            Self::PLUS_COLON => "PlusColon",
            Self::PLUS_DIV_MINUS => "PlusDivMinus",
            Self::PLUS_MOD_MINUS => "PlusModMinus",
            Self::MINUS => "Minus",
            Self::DOUBLE_MINUS => "DoubleMinus",
            Self::MINUS_COLON => "MinusColon",
            Self::MINUS_ARROW => "MinusArrow",
            Self::MINUS_DOUBLE_ARROW => "MinusDoubleArrow",
            Self::TILDE => "Tilde",
            Self::TILDE_AND => "TildeAnd",
            Self::TILDE_OR => "TildeOr",
            Self::TILDE_XOR => "TildeXor",
            Self::DOLLAR => "Dollar",
            Self::QUESTION => "Question",
            Self::HASH => "Hash",
            Self::DOUBLE_HASH => "DoubleHash",
            Self::HASH_MINUS_HASH => "HashMinusHash",
            Self::HASH_EQUALS_HASH => "HashEqualsHash",
            Self::XOR => "Xor",
            Self::XOR_TILDE => "XorTilde",
            Self::EQUALS => "Equals",
            Self::DOUBLE_EQUALS => "DoubleEquals",
            Self::DOUBLE_EQUALS_QUESTION => "DoubleEqualsQuestion",
            Self::TRIPLE_EQUALS => "TripleEquals",
            Self::EQUALS_ARROW => "EqualsArrow",
            Self::PLUS_EQUAL => "PlusEqual",
            Self::MINUS_EQUAL => "MinusEqual",
            Self::SLASH_EQUAL => "SlashEqual",
            Self::STAR_EQUAL => "StarEqual",
            Self::AND_EQUAL => "AndEqual",
            Self::OR_EQUAL => "OrEqual",
            Self::PERCENT_EQUAL => "PercentEqual",
            Self::XOR_EQUAL => "XorEqual",
            Self::LEFT_SHIFT_EQUAL => "LeftShiftEqual",
            Self::TRIPLE_LEFT_SHIFT_EQUAL => "TripleLeftShiftEqual",
            Self::RIGHT_SHIFT_EQUAL => "RightShiftEqual",
            Self::TRIPLE_RIGHT_SHIFT_EQUAL => "TripleRightShiftEqual",
            Self::LEFT_SHIFT => "LeftShift",
            Self::RIGHT_SHIFT => "RightShift",
            Self::TRIPLE_LEFT_SHIFT => "TripleLeftShift",
            Self::TRIPLE_RIGHT_SHIFT => "TripleRightShift",
            Self::EXCLAMATION => "Exclamation",
            Self::EXCLAMATION_EQUALS => "ExclamationEquals",
            Self::EXCLAMATION_EQUALS_QUESTION => "ExclamationEqualsQuestion",
            Self::EXCLAMATION_DOUBLE_EQUALS => "ExclamationDoubleEquals",
            Self::PERCENT => "Percent",
            Self::LESS_THAN => "LessThan",
            Self::LESS_THAN_EQUALS => "LessThanEquals",
            Self::LESS_THAN_MINUS_ARROW => "LessThanMinusArrow",
            Self::GREATER_THAN => "GreaterThan",
            Self::GREATER_THAN_EQUALS => "GreaterThanEquals",
            Self::OR => "Or",
            Self::DOUBLE_OR => "DoubleOr",
            Self::OR_MINUS_ARROW => "OrMinusArrow",
            Self::OR_EQUALS_ARROW => "OrEqualsArrow",
            Self::AT => "At",
            Self::DOUBLE_AT => "DoubleAt",
            Self::AND => "And",
            Self::DOUBLE_AND => "DoubleAnd",
            Self::TRIPLE_AND => "TripleAnd",
            Self::ONE_STEP => "OneStep",
            Self::ACCEPT_ON_KEYWORD => "AcceptOnKeyword",
            Self::ALIAS_KEYWORD => "AliasKeyword",
            Self::ALWAYS_KEYWORD => "AlwaysKeyword",
            Self::ALWAYS_COMB_KEYWORD => "AlwaysCombKeyword",
            Self::ALWAYS_FF_KEYWORD => "AlwaysFFKeyword",
            Self::ALWAYS_LATCH_KEYWORD => "AlwaysLatchKeyword",
            Self::AND_KEYWORD => "AndKeyword",
            Self::ASSERT_KEYWORD => "AssertKeyword",
            Self::ASSIGN_KEYWORD => "AssignKeyword",
            Self::ASSUME_KEYWORD => "AssumeKeyword",
            Self::AUTOMATIC_KEYWORD => "AutomaticKeyword",
            Self::BEFORE_KEYWORD => "BeforeKeyword",
            Self::BEGIN_KEYWORD => "BeginKeyword",
            Self::BIND_KEYWORD => "BindKeyword",
            Self::BINS_KEYWORD => "BinsKeyword",
            Self::BINS_OF_KEYWORD => "BinsOfKeyword",
            Self::BIT_KEYWORD => "BitKeyword",
            Self::BREAK_KEYWORD => "BreakKeyword",
            Self::BUF_KEYWORD => "BufKeyword",
            Self::BUF_IF_0_KEYWORD => "BufIf0Keyword",
            Self::BUF_IF_1_KEYWORD => "BufIf1Keyword",
            Self::BYTE_KEYWORD => "ByteKeyword",
            Self::CASE_KEYWORD => "CaseKeyword",
            Self::CASE_X_KEYWORD => "CaseXKeyword",
            Self::CASE_Z_KEYWORD => "CaseZKeyword",
            Self::CELL_KEYWORD => "CellKeyword",
            Self::C_HANDLE_KEYWORD => "CHandleKeyword",
            Self::CHECKER_KEYWORD => "CheckerKeyword",
            Self::CLASS_KEYWORD => "ClassKeyword",
            Self::CLOCKING_KEYWORD => "ClockingKeyword",
            Self::CMOS_KEYWORD => "CmosKeyword",
            Self::CONFIG_KEYWORD => "ConfigKeyword",
            Self::CONST_KEYWORD => "ConstKeyword",
            Self::CONSTRAINT_KEYWORD => "ConstraintKeyword",
            Self::CONTEXT_KEYWORD => "ContextKeyword",
            Self::CONTINUE_KEYWORD => "ContinueKeyword",
            Self::COVER_KEYWORD => "CoverKeyword",
            Self::COVER_GROUP_KEYWORD => "CoverGroupKeyword",
            Self::COVER_POINT_KEYWORD => "CoverPointKeyword",
            Self::CROSS_KEYWORD => "CrossKeyword",
            Self::DEASSIGN_KEYWORD => "DeassignKeyword",
            Self::DEFAULT_KEYWORD => "DefaultKeyword",
            Self::DEF_PARAM_KEYWORD => "DefParamKeyword",
            Self::DESIGN_KEYWORD => "DesignKeyword",
            Self::DISABLE_KEYWORD => "DisableKeyword",
            Self::DIST_KEYWORD => "DistKeyword",
            Self::DO_KEYWORD => "DoKeyword",
            Self::EDGE_KEYWORD => "EdgeKeyword",
            Self::ELSE_KEYWORD => "ElseKeyword",
            Self::END_KEYWORD => "EndKeyword",
            Self::END_CASE_KEYWORD => "EndCaseKeyword",
            Self::END_CHECKER_KEYWORD => "EndCheckerKeyword",
            Self::END_CLASS_KEYWORD => "EndClassKeyword",
            Self::END_CLOCKING_KEYWORD => "EndClockingKeyword",
            Self::END_CONFIG_KEYWORD => "EndConfigKeyword",
            Self::END_FUNCTION_KEYWORD => "EndFunctionKeyword",
            Self::END_GENERATE_KEYWORD => "EndGenerateKeyword",
            Self::END_GROUP_KEYWORD => "EndGroupKeyword",
            Self::END_INTERFACE_KEYWORD => "EndInterfaceKeyword",
            Self::END_MODULE_KEYWORD => "EndModuleKeyword",
            Self::END_PACKAGE_KEYWORD => "EndPackageKeyword",
            Self::END_PRIMITIVE_KEYWORD => "EndPrimitiveKeyword",
            Self::END_PROGRAM_KEYWORD => "EndProgramKeyword",
            Self::END_PROPERTY_KEYWORD => "EndPropertyKeyword",
            Self::END_SPECIFY_KEYWORD => "EndSpecifyKeyword",
            Self::END_SEQUENCE_KEYWORD => "EndSequenceKeyword",
            Self::END_TABLE_KEYWORD => "EndTableKeyword",
            Self::END_TASK_KEYWORD => "EndTaskKeyword",
            Self::ENUM_KEYWORD => "EnumKeyword",
            Self::EVENT_KEYWORD => "EventKeyword",
            Self::EVENTUALLY_KEYWORD => "EventuallyKeyword",
            Self::EXPECT_KEYWORD => "ExpectKeyword",
            Self::EXPORT_KEYWORD => "ExportKeyword",
            Self::EXTENDS_KEYWORD => "ExtendsKeyword",
            Self::EXTERN_KEYWORD => "ExternKeyword",
            Self::FINAL_KEYWORD => "FinalKeyword",
            Self::FIRST_MATCH_KEYWORD => "FirstMatchKeyword",
            Self::FOR_KEYWORD => "ForKeyword",
            Self::FORCE_KEYWORD => "ForceKeyword",
            Self::FOREACH_KEYWORD => "ForeachKeyword",
            Self::FOREVER_KEYWORD => "ForeverKeyword",
            Self::FORK_KEYWORD => "ForkKeyword",
            Self::FORK_JOIN_KEYWORD => "ForkJoinKeyword",
            Self::FUNCTION_KEYWORD => "FunctionKeyword",
            Self::GENERATE_KEYWORD => "GenerateKeyword",
            Self::GEN_VAR_KEYWORD => "GenVarKeyword",
            Self::GLOBAL_KEYWORD => "GlobalKeyword",
            Self::HIGH_Z0_KEYWORD => "HighZ0Keyword",
            Self::HIGH_Z1_KEYWORD => "HighZ1Keyword",
            Self::IF_KEYWORD => "IfKeyword",
            Self::IFF_KEYWORD => "IffKeyword",
            Self::IF_NONE_KEYWORD => "IfNoneKeyword",
            Self::IGNORE_BINS_KEYWORD => "IgnoreBinsKeyword",
            Self::ILLEGAL_BINS_KEYWORD => "IllegalBinsKeyword",
            Self::IMPLEMENTS_KEYWORD => "ImplementsKeyword",
            Self::IMPLIES_KEYWORD => "ImpliesKeyword",
            Self::IMPORT_KEYWORD => "ImportKeyword",
            Self::INC_DIR_KEYWORD => "IncDirKeyword",
            Self::INCLUDE_KEYWORD => "IncludeKeyword",
            Self::INITIAL_KEYWORD => "InitialKeyword",
            Self::IN_OUT_KEYWORD => "InOutKeyword",
            Self::INPUT_KEYWORD => "InputKeyword",
            Self::INSIDE_KEYWORD => "InsideKeyword",
            Self::INSTANCE_KEYWORD => "InstanceKeyword",
            Self::INT_KEYWORD => "IntKeyword",
            Self::INTEGER_KEYWORD => "IntegerKeyword",
            Self::INTERCONNECT_KEYWORD => "InterconnectKeyword",
            Self::INTERFACE_KEYWORD => "InterfaceKeyword",
            Self::INTERSECT_KEYWORD => "IntersectKeyword",
            Self::JOIN_KEYWORD => "JoinKeyword",
            Self::JOIN_ANY_KEYWORD => "JoinAnyKeyword",
            Self::JOIN_NONE_KEYWORD => "JoinNoneKeyword",
            Self::LARGE_KEYWORD => "LargeKeyword",
            Self::LET_KEYWORD => "LetKeyword",
            Self::LIB_LIST_KEYWORD => "LibListKeyword",
            Self::LIBRARY_KEYWORD => "LibraryKeyword",
            Self::LOCAL_KEYWORD => "LocalKeyword",
            Self::LOCAL_PARAM_KEYWORD => "LocalParamKeyword",
            Self::LOGIC_KEYWORD => "LogicKeyword",
            Self::LONG_INT_KEYWORD => "LongIntKeyword",
            Self::MACROMODULE_KEYWORD => "MacromoduleKeyword",
            Self::MATCHES_KEYWORD => "MatchesKeyword",
            Self::MEDIUM_KEYWORD => "MediumKeyword",
            Self::MOD_PORT_KEYWORD => "ModPortKeyword",
            Self::MODULE_KEYWORD => "ModuleKeyword",
            Self::NAND_KEYWORD => "NandKeyword",
            Self::NEG_EDGE_KEYWORD => "NegEdgeKeyword",
            Self::NET_TYPE_KEYWORD => "NetTypeKeyword",
            Self::NEW_KEYWORD => "NewKeyword",
            Self::NEXT_TIME_KEYWORD => "NextTimeKeyword",
            Self::NMOS_KEYWORD => "NmosKeyword",
            Self::NOR_KEYWORD => "NorKeyword",
            Self::NO_SHOW_CANCELLED_KEYWORD => "NoShowCancelledKeyword",
            Self::NOT_KEYWORD => "NotKeyword",
            Self::NOT_IF_0_KEYWORD => "NotIf0Keyword",
            Self::NOT_IF_1_KEYWORD => "NotIf1Keyword",
            Self::NULL_KEYWORD => "NullKeyword",
            Self::OR_KEYWORD => "OrKeyword",
            Self::OUTPUT_KEYWORD => "OutputKeyword",
            Self::PACKAGE_KEYWORD => "PackageKeyword",
            Self::PACKED_KEYWORD => "PackedKeyword",
            Self::PARAMETER_KEYWORD => "ParameterKeyword",
            Self::PMOS_KEYWORD => "PmosKeyword",
            Self::POS_EDGE_KEYWORD => "PosEdgeKeyword",
            Self::PRIMITIVE_KEYWORD => "PrimitiveKeyword",
            Self::PRIORITY_KEYWORD => "PriorityKeyword",
            Self::PROGRAM_KEYWORD => "ProgramKeyword",
            Self::PROPERTY_KEYWORD => "PropertyKeyword",
            Self::PROTECTED_KEYWORD => "ProtectedKeyword",
            Self::PULL_0_KEYWORD => "Pull0Keyword",
            Self::PULL_1_KEYWORD => "Pull1Keyword",
            Self::PULL_DOWN_KEYWORD => "PullDownKeyword",
            Self::PULL_UP_KEYWORD => "PullUpKeyword",
            Self::PULSE_STYLE_ON_DETECT_KEYWORD => "PulseStyleOnDetectKeyword",
            Self::PULSE_STYLE_ON_EVENT_KEYWORD => "PulseStyleOnEventKeyword",
            Self::PURE_KEYWORD => "PureKeyword",
            Self::RAND_KEYWORD => "RandKeyword",
            Self::RAND_C_KEYWORD => "RandCKeyword",
            Self::RAND_CASE_KEYWORD => "RandCaseKeyword",
            Self::RAND_SEQUENCE_KEYWORD => "RandSequenceKeyword",
            Self::RCMOS_KEYWORD => "RcmosKeyword",
            Self::REAL_KEYWORD => "RealKeyword",
            Self::REAL_TIME_KEYWORD => "RealTimeKeyword",
            Self::REF_KEYWORD => "RefKeyword",
            Self::REG_KEYWORD => "RegKeyword",
            Self::REJECT_ON_KEYWORD => "RejectOnKeyword",
            Self::RELEASE_KEYWORD => "ReleaseKeyword",
            Self::REPEAT_KEYWORD => "RepeatKeyword",
            Self::RESTRICT_KEYWORD => "RestrictKeyword",
            Self::RETURN_KEYWORD => "ReturnKeyword",
            Self::RNMOS_KEYWORD => "RnmosKeyword",
            Self::RPMOS_KEYWORD => "RpmosKeyword",
            Self::RTRAN_KEYWORD => "RtranKeyword",
            Self::RTRAN_IF_0_KEYWORD => "RtranIf0Keyword",
            Self::RTRAN_IF_1_KEYWORD => "RtranIf1Keyword",
            Self::S_ALWAYS_KEYWORD => "SAlwaysKeyword",
            Self::S_EVENTUALLY_KEYWORD => "SEventuallyKeyword",
            Self::S_NEXT_TIME_KEYWORD => "SNextTimeKeyword",
            Self::S_UNTIL_KEYWORD => "SUntilKeyword",
            Self::S_UNTIL_WITH_KEYWORD => "SUntilWithKeyword",
            Self::SCALARED_KEYWORD => "ScalaredKeyword",
            Self::SEQUENCE_KEYWORD => "SequenceKeyword",
            Self::SHORT_INT_KEYWORD => "ShortIntKeyword",
            Self::SHORT_REAL_KEYWORD => "ShortRealKeyword",
            Self::SHOW_CANCELLED_KEYWORD => "ShowCancelledKeyword",
            Self::SIGNED_KEYWORD => "SignedKeyword",
            Self::SMALL_KEYWORD => "SmallKeyword",
            Self::SOFT_KEYWORD => "SoftKeyword",
            Self::SOLVE_KEYWORD => "SolveKeyword",
            Self::SPECIFY_KEYWORD => "SpecifyKeyword",
            Self::SPEC_PARAM_KEYWORD => "SpecParamKeyword",
            Self::STATIC_KEYWORD => "StaticKeyword",
            Self::STRING_KEYWORD => "StringKeyword",
            Self::STRONG_KEYWORD => "StrongKeyword",
            Self::STRONG_0_KEYWORD => "Strong0Keyword",
            Self::STRONG_1_KEYWORD => "Strong1Keyword",
            Self::STRUCT_KEYWORD => "StructKeyword",
            Self::SUPER_KEYWORD => "SuperKeyword",
            Self::SUPPLY_0_KEYWORD => "Supply0Keyword",
            Self::SUPPLY_1_KEYWORD => "Supply1Keyword",
            Self::SYNC_ACCEPT_ON_KEYWORD => "SyncAcceptOnKeyword",
            Self::SYNC_REJECT_ON_KEYWORD => "SyncRejectOnKeyword",
            Self::TABLE_KEYWORD => "TableKeyword",
            Self::TAGGED_KEYWORD => "TaggedKeyword",
            Self::TASK_KEYWORD => "TaskKeyword",
            Self::THIS_KEYWORD => "ThisKeyword",
            Self::THROUGHOUT_KEYWORD => "ThroughoutKeyword",
            Self::TIME_KEYWORD => "TimeKeyword",
            Self::TIME_PRECISION_KEYWORD => "TimePrecisionKeyword",
            Self::TIME_UNIT_KEYWORD => "TimeUnitKeyword",
            Self::TRAN_KEYWORD => "TranKeyword",
            Self::TRAN_IF_0_KEYWORD => "TranIf0Keyword",
            Self::TRAN_IF_1_KEYWORD => "TranIf1Keyword",
            Self::TRI_KEYWORD => "TriKeyword",
            Self::TRI_0_KEYWORD => "Tri0Keyword",
            Self::TRI_1_KEYWORD => "Tri1Keyword",
            Self::TRI_AND_KEYWORD => "TriAndKeyword",
            Self::TRI_OR_KEYWORD => "TriOrKeyword",
            Self::TRI_REG_KEYWORD => "TriRegKeyword",
            Self::TYPE_KEYWORD => "TypeKeyword",
            Self::TYPEDEF_KEYWORD => "TypedefKeyword",
            Self::UNION_KEYWORD => "UnionKeyword",
            Self::UNIQUE_KEYWORD => "UniqueKeyword",
            Self::UNIQUE_0_KEYWORD => "Unique0Keyword",
            Self::UNSIGNED_KEYWORD => "UnsignedKeyword",
            Self::UNTIL_KEYWORD => "UntilKeyword",
            Self::UNTIL_WITH_KEYWORD => "UntilWithKeyword",
            Self::UNTYPED_KEYWORD => "UntypedKeyword",
            Self::USE_KEYWORD => "UseKeyword",
            Self::U_WIRE_KEYWORD => "UWireKeyword",
            Self::VAR_KEYWORD => "VarKeyword",
            Self::VECTORED_KEYWORD => "VectoredKeyword",
            Self::VIRTUAL_KEYWORD => "VirtualKeyword",
            Self::VOID_KEYWORD => "VoidKeyword",
            Self::WAIT_KEYWORD => "WaitKeyword",
            Self::WAIT_ORDER_KEYWORD => "WaitOrderKeyword",
            Self::W_AND_KEYWORD => "WAndKeyword",
            Self::WEAK_KEYWORD => "WeakKeyword",
            Self::WEAK_0_KEYWORD => "Weak0Keyword",
            Self::WEAK_1_KEYWORD => "Weak1Keyword",
            Self::WHILE_KEYWORD => "WhileKeyword",
            Self::WILDCARD_KEYWORD => "WildcardKeyword",
            Self::WIRE_KEYWORD => "WireKeyword",
            Self::WITH_KEYWORD => "WithKeyword",
            Self::WITHIN_KEYWORD => "WithinKeyword",
            Self::W_OR_KEYWORD => "WOrKeyword",
            Self::XNOR_KEYWORD => "XnorKeyword",
            Self::XOR_KEYWORD => "XorKeyword",
            Self::UNIT_SYSTEM_NAME => "UnitSystemName",
            Self::ROOT_SYSTEM_NAME => "RootSystemName",
            Self::DIRECTIVE => "Directive",
            Self::INCLUDE_FILE_NAME => "IncludeFileName",
            Self::MACRO_USAGE => "MacroUsage",
            Self::MACRO_QUOTE => "MacroQuote",
            Self::MACRO_TRIPLE_QUOTE => "MacroTripleQuote",
            Self::MACRO_ESCAPED_QUOTE => "MacroEscapedQuote",
            Self::MACRO_PASTE => "MacroPaste",
            Self::EMPTY_MACRO_ARGUMENT => "EmptyMacroArgument",
            Self::LINE_CONTINUATION => "LineContinuation",
            _ => unreachable!(),
        };
        f.write_str(name)
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TriviaKind(u8);
impl TriviaKind {
    pub const BLOCK_COMMENT: Self = Self(4u8);
    pub const DIRECTIVE: Self = Self(8u8);
    pub const DISABLED_TEXT: Self = Self(5u8);
    pub const END_OF_LINE: Self = Self(2u8);
    pub const LINE_COMMENT: Self = Self(3u8);
    pub const SKIPPED_SYNTAX: Self = Self(7u8);
    pub const SKIPPED_TOKENS: Self = Self(6u8);
    pub const UNKNOWN: Self = Self(0u8);
    pub const WHITESPACE: Self = Self(1u8);

    pub fn from_id(id: u8) -> Self {
        Self(id)
    }
}
impl fmt::Debug for TriviaKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match *self {
            Self::UNKNOWN => "Unknown",
            Self::WHITESPACE => "Whitespace",
            Self::END_OF_LINE => "EndOfLine",
            Self::LINE_COMMENT => "LineComment",
            Self::BLOCK_COMMENT => "BlockComment",
            Self::DISABLED_TEXT => "DisabledText",
            Self::SKIPPED_TOKENS => "SkippedTokens",
            Self::SKIPPED_SYNTAX => "SkippedSyntax",
            Self::DIRECTIVE => "Directive",
            _ => unreachable!(),
        };
        f.write_str(name)
    }
}
