pub(crate) fn owned_node_kind(kind: slang::SyntaxKind) -> syntax::SyntaxKind {
    match kind {
        slang::SyntaxKind::ACCEPT_ON_PROPERTY_EXPR => syntax::SyntaxKind::ACCEPT_ON_PROPERTY_EXPR,
        slang::SyntaxKind::ACTION_BLOCK => syntax::SyntaxKind::ACTION_BLOCK,
        slang::SyntaxKind::ADD_ASSIGNMENT_EXPRESSION => {
            syntax::SyntaxKind::ADD_ASSIGNMENT_EXPRESSION
        }
        slang::SyntaxKind::ADD_EXPRESSION => syntax::SyntaxKind::ADD_EXPRESSION,
        slang::SyntaxKind::ALWAYS_BLOCK => syntax::SyntaxKind::ALWAYS_BLOCK,
        slang::SyntaxKind::ALWAYS_COMB_BLOCK => syntax::SyntaxKind::ALWAYS_COMB_BLOCK,
        slang::SyntaxKind::ALWAYS_FF_BLOCK => syntax::SyntaxKind::ALWAYS_FF_BLOCK,
        slang::SyntaxKind::ALWAYS_LATCH_BLOCK => syntax::SyntaxKind::ALWAYS_LATCH_BLOCK,
        slang::SyntaxKind::AND_ASSIGNMENT_EXPRESSION => {
            syntax::SyntaxKind::AND_ASSIGNMENT_EXPRESSION
        }
        slang::SyntaxKind::AND_PROPERTY_EXPR => syntax::SyntaxKind::AND_PROPERTY_EXPR,
        slang::SyntaxKind::AND_SEQUENCE_EXPR => syntax::SyntaxKind::AND_SEQUENCE_EXPR,
        slang::SyntaxKind::ANONYMOUS_PROGRAM => syntax::SyntaxKind::ANONYMOUS_PROGRAM,
        slang::SyntaxKind::ANSI_PORT_LIST => syntax::SyntaxKind::ANSI_PORT_LIST,
        slang::SyntaxKind::ANSI_UDP_PORT_LIST => syntax::SyntaxKind::ANSI_UDP_PORT_LIST,
        slang::SyntaxKind::ARGUMENT_LIST => syntax::SyntaxKind::ARGUMENT_LIST,
        slang::SyntaxKind::ARITHMETIC_LEFT_SHIFT_ASSIGNMENT_EXPRESSION => {
            syntax::SyntaxKind::ARITHMETIC_LEFT_SHIFT_ASSIGNMENT_EXPRESSION
        }
        slang::SyntaxKind::ARITHMETIC_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION => {
            syntax::SyntaxKind::ARITHMETIC_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION
        }
        slang::SyntaxKind::ARITHMETIC_SHIFT_LEFT_EXPRESSION => {
            syntax::SyntaxKind::ARITHMETIC_SHIFT_LEFT_EXPRESSION
        }
        slang::SyntaxKind::ARITHMETIC_SHIFT_RIGHT_EXPRESSION => {
            syntax::SyntaxKind::ARITHMETIC_SHIFT_RIGHT_EXPRESSION
        }
        slang::SyntaxKind::ARRAY_AND_METHOD => syntax::SyntaxKind::ARRAY_AND_METHOD,
        slang::SyntaxKind::ARRAY_OR_METHOD => syntax::SyntaxKind::ARRAY_OR_METHOD,
        slang::SyntaxKind::ARRAY_OR_RANDOMIZE_METHOD_EXPRESSION => {
            syntax::SyntaxKind::ARRAY_OR_RANDOMIZE_METHOD_EXPRESSION
        }
        slang::SyntaxKind::ARRAY_UNIQUE_METHOD => syntax::SyntaxKind::ARRAY_UNIQUE_METHOD,
        slang::SyntaxKind::ARRAY_XOR_METHOD => syntax::SyntaxKind::ARRAY_XOR_METHOD,
        slang::SyntaxKind::ASCENDING_RANGE_SELECT => syntax::SyntaxKind::ASCENDING_RANGE_SELECT,
        slang::SyntaxKind::ASSERT_PROPERTY_STATEMENT => {
            syntax::SyntaxKind::ASSERT_PROPERTY_STATEMENT
        }
        slang::SyntaxKind::ASSERTION_ITEM_PORT => syntax::SyntaxKind::ASSERTION_ITEM_PORT,
        slang::SyntaxKind::ASSERTION_ITEM_PORT_LIST => syntax::SyntaxKind::ASSERTION_ITEM_PORT_LIST,
        slang::SyntaxKind::ASSIGNMENT_EXPRESSION => syntax::SyntaxKind::ASSIGNMENT_EXPRESSION,
        slang::SyntaxKind::ASSIGNMENT_PATTERN_EXPRESSION => {
            syntax::SyntaxKind::ASSIGNMENT_PATTERN_EXPRESSION
        }
        slang::SyntaxKind::ASSIGNMENT_PATTERN_ITEM => syntax::SyntaxKind::ASSIGNMENT_PATTERN_ITEM,
        slang::SyntaxKind::ASSUME_PROPERTY_STATEMENT => {
            syntax::SyntaxKind::ASSUME_PROPERTY_STATEMENT
        }
        slang::SyntaxKind::ATTRIBUTE_INSTANCE => syntax::SyntaxKind::ATTRIBUTE_INSTANCE,
        slang::SyntaxKind::ATTRIBUTE_SPEC => syntax::SyntaxKind::ATTRIBUTE_SPEC,
        slang::SyntaxKind::BAD_EXPRESSION => syntax::SyntaxKind::BAD_EXPRESSION,
        slang::SyntaxKind::BEGIN_KEYWORDS_DIRECTIVE => syntax::SyntaxKind::BEGIN_KEYWORDS_DIRECTIVE,
        slang::SyntaxKind::BIN_SELECT_WITH_FILTER_EXPR => {
            syntax::SyntaxKind::BIN_SELECT_WITH_FILTER_EXPR
        }
        slang::SyntaxKind::BINARY_AND_EXPRESSION => syntax::SyntaxKind::BINARY_AND_EXPRESSION,
        slang::SyntaxKind::BINARY_BINS_SELECT_EXPR => syntax::SyntaxKind::BINARY_BINS_SELECT_EXPR,
        slang::SyntaxKind::BINARY_BLOCK_EVENT_EXPRESSION => {
            syntax::SyntaxKind::BINARY_BLOCK_EVENT_EXPRESSION
        }
        slang::SyntaxKind::BINARY_CONDITIONAL_DIRECTIVE_EXPRESSION => {
            syntax::SyntaxKind::BINARY_CONDITIONAL_DIRECTIVE_EXPRESSION
        }
        slang::SyntaxKind::BINARY_EVENT_EXPRESSION => syntax::SyntaxKind::BINARY_EVENT_EXPRESSION,
        slang::SyntaxKind::BINARY_OR_EXPRESSION => syntax::SyntaxKind::BINARY_OR_EXPRESSION,
        slang::SyntaxKind::BINARY_XNOR_EXPRESSION => syntax::SyntaxKind::BINARY_XNOR_EXPRESSION,
        slang::SyntaxKind::BINARY_XOR_EXPRESSION => syntax::SyntaxKind::BINARY_XOR_EXPRESSION,
        slang::SyntaxKind::BIND_DIRECTIVE => syntax::SyntaxKind::BIND_DIRECTIVE,
        slang::SyntaxKind::BIND_TARGET_LIST => syntax::SyntaxKind::BIND_TARGET_LIST,
        slang::SyntaxKind::BINS_SELECT_CONDITION_EXPR => {
            syntax::SyntaxKind::BINS_SELECT_CONDITION_EXPR
        }
        slang::SyntaxKind::BINS_SELECTION => syntax::SyntaxKind::BINS_SELECTION,
        slang::SyntaxKind::BIT_SELECT => syntax::SyntaxKind::BIT_SELECT,
        slang::SyntaxKind::BIT_TYPE => syntax::SyntaxKind::BIT_TYPE,
        slang::SyntaxKind::BLOCK_COVERAGE_EVENT => syntax::SyntaxKind::BLOCK_COVERAGE_EVENT,
        slang::SyntaxKind::BLOCKING_EVENT_TRIGGER_STATEMENT => {
            syntax::SyntaxKind::BLOCKING_EVENT_TRIGGER_STATEMENT
        }
        slang::SyntaxKind::BYTE_TYPE => syntax::SyntaxKind::BYTE_TYPE,
        slang::SyntaxKind::C_HANDLE_TYPE => syntax::SyntaxKind::C_HANDLE_TYPE,
        slang::SyntaxKind::CASE_EQUALITY_EXPRESSION => syntax::SyntaxKind::CASE_EQUALITY_EXPRESSION,
        slang::SyntaxKind::CASE_GENERATE => syntax::SyntaxKind::CASE_GENERATE,
        slang::SyntaxKind::CASE_INEQUALITY_EXPRESSION => {
            syntax::SyntaxKind::CASE_INEQUALITY_EXPRESSION
        }
        slang::SyntaxKind::CASE_PROPERTY_EXPR => syntax::SyntaxKind::CASE_PROPERTY_EXPR,
        slang::SyntaxKind::CASE_STATEMENT => syntax::SyntaxKind::CASE_STATEMENT,
        slang::SyntaxKind::CAST_EXPRESSION => syntax::SyntaxKind::CAST_EXPRESSION,
        slang::SyntaxKind::CELL_CONFIG_RULE => syntax::SyntaxKind::CELL_CONFIG_RULE,
        slang::SyntaxKind::CELL_DEFINE_DIRECTIVE => syntax::SyntaxKind::CELL_DEFINE_DIRECTIVE,
        slang::SyntaxKind::CHARGE_STRENGTH => syntax::SyntaxKind::CHARGE_STRENGTH,
        slang::SyntaxKind::CHECKER_DATA_DECLARATION => syntax::SyntaxKind::CHECKER_DATA_DECLARATION,
        slang::SyntaxKind::CHECKER_DECLARATION => syntax::SyntaxKind::CHECKER_DECLARATION,
        slang::SyntaxKind::CHECKER_INSTANCE_STATEMENT => {
            syntax::SyntaxKind::CHECKER_INSTANCE_STATEMENT
        }
        slang::SyntaxKind::CHECKER_INSTANTIATION => syntax::SyntaxKind::CHECKER_INSTANTIATION,
        slang::SyntaxKind::CLASS_DECLARATION => syntax::SyntaxKind::CLASS_DECLARATION,
        slang::SyntaxKind::CLASS_METHOD_DECLARATION => syntax::SyntaxKind::CLASS_METHOD_DECLARATION,
        slang::SyntaxKind::CLASS_METHOD_PROTOTYPE => syntax::SyntaxKind::CLASS_METHOD_PROTOTYPE,
        slang::SyntaxKind::CLASS_NAME => syntax::SyntaxKind::CLASS_NAME,
        slang::SyntaxKind::CLASS_PROPERTY_DECLARATION => {
            syntax::SyntaxKind::CLASS_PROPERTY_DECLARATION
        }
        slang::SyntaxKind::CLASS_SPECIFIER => syntax::SyntaxKind::CLASS_SPECIFIER,
        slang::SyntaxKind::CLOCKING_DECLARATION => syntax::SyntaxKind::CLOCKING_DECLARATION,
        slang::SyntaxKind::CLOCKING_DIRECTION => syntax::SyntaxKind::CLOCKING_DIRECTION,
        slang::SyntaxKind::CLOCKING_ITEM => syntax::SyntaxKind::CLOCKING_ITEM,
        slang::SyntaxKind::CLOCKING_PROPERTY_EXPR => syntax::SyntaxKind::CLOCKING_PROPERTY_EXPR,
        slang::SyntaxKind::CLOCKING_SEQUENCE_EXPR => syntax::SyntaxKind::CLOCKING_SEQUENCE_EXPR,
        slang::SyntaxKind::CLOCKING_SKEW => syntax::SyntaxKind::CLOCKING_SKEW,
        slang::SyntaxKind::COLON_EXPRESSION_CLAUSE => syntax::SyntaxKind::COLON_EXPRESSION_CLAUSE,
        slang::SyntaxKind::COMPILATION_UNIT => syntax::SyntaxKind::COMPILATION_UNIT,
        slang::SyntaxKind::CONCATENATION_EXPRESSION => syntax::SyntaxKind::CONCATENATION_EXPRESSION,
        slang::SyntaxKind::CONCURRENT_ASSERTION_MEMBER => {
            syntax::SyntaxKind::CONCURRENT_ASSERTION_MEMBER
        }
        slang::SyntaxKind::CONDITIONAL_CONSTRAINT => syntax::SyntaxKind::CONDITIONAL_CONSTRAINT,
        slang::SyntaxKind::CONDITIONAL_EXPRESSION => syntax::SyntaxKind::CONDITIONAL_EXPRESSION,
        slang::SyntaxKind::CONDITIONAL_PATH_DECLARATION => {
            syntax::SyntaxKind::CONDITIONAL_PATH_DECLARATION
        }
        slang::SyntaxKind::CONDITIONAL_PATTERN => syntax::SyntaxKind::CONDITIONAL_PATTERN,
        slang::SyntaxKind::CONDITIONAL_PREDICATE => syntax::SyntaxKind::CONDITIONAL_PREDICATE,
        slang::SyntaxKind::CONDITIONAL_PROPERTY_EXPR => {
            syntax::SyntaxKind::CONDITIONAL_PROPERTY_EXPR
        }
        slang::SyntaxKind::CONDITIONAL_STATEMENT => syntax::SyntaxKind::CONDITIONAL_STATEMENT,
        slang::SyntaxKind::CONFIG_CELL_IDENTIFIER => syntax::SyntaxKind::CONFIG_CELL_IDENTIFIER,
        slang::SyntaxKind::CONFIG_DECLARATION => syntax::SyntaxKind::CONFIG_DECLARATION,
        slang::SyntaxKind::CONFIG_INSTANCE_IDENTIFIER => {
            syntax::SyntaxKind::CONFIG_INSTANCE_IDENTIFIER
        }
        slang::SyntaxKind::CONFIG_LIBLIST => syntax::SyntaxKind::CONFIG_LIBLIST,
        slang::SyntaxKind::CONFIG_USE_CLAUSE => syntax::SyntaxKind::CONFIG_USE_CLAUSE,
        slang::SyntaxKind::CONSTRAINT_BLOCK => syntax::SyntaxKind::CONSTRAINT_BLOCK,
        slang::SyntaxKind::CONSTRAINT_DECLARATION => syntax::SyntaxKind::CONSTRAINT_DECLARATION,
        slang::SyntaxKind::CONSTRAINT_PROTOTYPE => syntax::SyntaxKind::CONSTRAINT_PROTOTYPE,
        slang::SyntaxKind::CONSTRUCTOR_NAME => syntax::SyntaxKind::CONSTRUCTOR_NAME,
        slang::SyntaxKind::CONTINUOUS_ASSIGN => syntax::SyntaxKind::CONTINUOUS_ASSIGN,
        slang::SyntaxKind::COPY_CLASS_EXPRESSION => syntax::SyntaxKind::COPY_CLASS_EXPRESSION,
        slang::SyntaxKind::COVER_CROSS => syntax::SyntaxKind::COVER_CROSS,
        slang::SyntaxKind::COVER_PROPERTY_STATEMENT => syntax::SyntaxKind::COVER_PROPERTY_STATEMENT,
        slang::SyntaxKind::COVER_SEQUENCE_STATEMENT => syntax::SyntaxKind::COVER_SEQUENCE_STATEMENT,
        slang::SyntaxKind::COVERAGE_BINS => syntax::SyntaxKind::COVERAGE_BINS,
        slang::SyntaxKind::COVERAGE_BINS_ARRAY_SIZE => syntax::SyntaxKind::COVERAGE_BINS_ARRAY_SIZE,
        slang::SyntaxKind::COVERAGE_IFF_CLAUSE => syntax::SyntaxKind::COVERAGE_IFF_CLAUSE,
        slang::SyntaxKind::COVERAGE_OPTION => syntax::SyntaxKind::COVERAGE_OPTION,
        slang::SyntaxKind::COVERGROUP_DECLARATION => syntax::SyntaxKind::COVERGROUP_DECLARATION,
        slang::SyntaxKind::COVERPOINT => syntax::SyntaxKind::COVERPOINT,
        slang::SyntaxKind::CYCLE_DELAY => syntax::SyntaxKind::CYCLE_DELAY,
        slang::SyntaxKind::DATA_DECLARATION => syntax::SyntaxKind::DATA_DECLARATION,
        slang::SyntaxKind::DECLARATOR => syntax::SyntaxKind::DECLARATOR,
        slang::SyntaxKind::DEF_PARAM => syntax::SyntaxKind::DEF_PARAM,
        slang::SyntaxKind::DEF_PARAM_ASSIGNMENT => syntax::SyntaxKind::DEF_PARAM_ASSIGNMENT,
        slang::SyntaxKind::DEFAULT_CASE_ITEM => syntax::SyntaxKind::DEFAULT_CASE_ITEM,
        slang::SyntaxKind::DEFAULT_CLOCKING_REFERENCE => {
            syntax::SyntaxKind::DEFAULT_CLOCKING_REFERENCE
        }
        slang::SyntaxKind::DEFAULT_CONFIG_RULE => syntax::SyntaxKind::DEFAULT_CONFIG_RULE,
        slang::SyntaxKind::DEFAULT_COVERAGE_BIN_INITIALIZER => {
            syntax::SyntaxKind::DEFAULT_COVERAGE_BIN_INITIALIZER
        }
        slang::SyntaxKind::DEFAULT_DECAY_TIME_DIRECTIVE => {
            syntax::SyntaxKind::DEFAULT_DECAY_TIME_DIRECTIVE
        }
        slang::SyntaxKind::DEFAULT_DISABLE_DECLARATION => {
            syntax::SyntaxKind::DEFAULT_DISABLE_DECLARATION
        }
        slang::SyntaxKind::DEFAULT_DIST_ITEM => syntax::SyntaxKind::DEFAULT_DIST_ITEM,
        slang::SyntaxKind::DEFAULT_EXTENDS_CLAUSE_ARG => {
            syntax::SyntaxKind::DEFAULT_EXTENDS_CLAUSE_ARG
        }
        slang::SyntaxKind::DEFAULT_FUNCTION_PORT => syntax::SyntaxKind::DEFAULT_FUNCTION_PORT,
        slang::SyntaxKind::DEFAULT_NET_TYPE_DIRECTIVE => {
            syntax::SyntaxKind::DEFAULT_NET_TYPE_DIRECTIVE
        }
        slang::SyntaxKind::DEFAULT_PATTERN_KEY_EXPRESSION => {
            syntax::SyntaxKind::DEFAULT_PATTERN_KEY_EXPRESSION
        }
        slang::SyntaxKind::DEFAULT_PROPERTY_CASE_ITEM => {
            syntax::SyntaxKind::DEFAULT_PROPERTY_CASE_ITEM
        }
        slang::SyntaxKind::DEFAULT_RS_CASE_ITEM => syntax::SyntaxKind::DEFAULT_RS_CASE_ITEM,
        slang::SyntaxKind::DEFAULT_SKEW_ITEM => syntax::SyntaxKind::DEFAULT_SKEW_ITEM,
        slang::SyntaxKind::DEFAULT_TRIREG_STRENGTH_DIRECTIVE => {
            syntax::SyntaxKind::DEFAULT_TRIREG_STRENGTH_DIRECTIVE
        }
        slang::SyntaxKind::DEFERRED_ASSERTION => syntax::SyntaxKind::DEFERRED_ASSERTION,
        slang::SyntaxKind::DEFINE_DIRECTIVE => syntax::SyntaxKind::DEFINE_DIRECTIVE,
        slang::SyntaxKind::DELAY_3 => syntax::SyntaxKind::DELAY_3,
        slang::SyntaxKind::DELAY_CONTROL => syntax::SyntaxKind::DELAY_CONTROL,
        slang::SyntaxKind::DELAY_MODE_DISTRIBUTED_DIRECTIVE => {
            syntax::SyntaxKind::DELAY_MODE_DISTRIBUTED_DIRECTIVE
        }
        slang::SyntaxKind::DELAY_MODE_PATH_DIRECTIVE => {
            syntax::SyntaxKind::DELAY_MODE_PATH_DIRECTIVE
        }
        slang::SyntaxKind::DELAY_MODE_UNIT_DIRECTIVE => {
            syntax::SyntaxKind::DELAY_MODE_UNIT_DIRECTIVE
        }
        slang::SyntaxKind::DELAY_MODE_ZERO_DIRECTIVE => {
            syntax::SyntaxKind::DELAY_MODE_ZERO_DIRECTIVE
        }
        slang::SyntaxKind::DELAYED_SEQUENCE_ELEMENT => syntax::SyntaxKind::DELAYED_SEQUENCE_ELEMENT,
        slang::SyntaxKind::DELAYED_SEQUENCE_EXPR => syntax::SyntaxKind::DELAYED_SEQUENCE_EXPR,
        slang::SyntaxKind::DESCENDING_RANGE_SELECT => syntax::SyntaxKind::DESCENDING_RANGE_SELECT,
        slang::SyntaxKind::DISABLE_CONSTRAINT => syntax::SyntaxKind::DISABLE_CONSTRAINT,
        slang::SyntaxKind::DISABLE_FORK_STATEMENT => syntax::SyntaxKind::DISABLE_FORK_STATEMENT,
        slang::SyntaxKind::DISABLE_IFF => syntax::SyntaxKind::DISABLE_IFF,
        slang::SyntaxKind::DISABLE_STATEMENT => syntax::SyntaxKind::DISABLE_STATEMENT,
        slang::SyntaxKind::DIST_CONSTRAINT_LIST => syntax::SyntaxKind::DIST_CONSTRAINT_LIST,
        slang::SyntaxKind::DIST_ITEM => syntax::SyntaxKind::DIST_ITEM,
        slang::SyntaxKind::DIST_WEIGHT => syntax::SyntaxKind::DIST_WEIGHT,
        slang::SyntaxKind::DIVIDE_ASSIGNMENT_EXPRESSION => {
            syntax::SyntaxKind::DIVIDE_ASSIGNMENT_EXPRESSION
        }
        slang::SyntaxKind::DIVIDE_EXPRESSION => syntax::SyntaxKind::DIVIDE_EXPRESSION,
        slang::SyntaxKind::DIVIDER_CLAUSE => syntax::SyntaxKind::DIVIDER_CLAUSE,
        slang::SyntaxKind::DO_WHILE_STATEMENT => syntax::SyntaxKind::DO_WHILE_STATEMENT,
        slang::SyntaxKind::DOT_MEMBER_CLAUSE => syntax::SyntaxKind::DOT_MEMBER_CLAUSE,
        slang::SyntaxKind::DPI_EXPORT => syntax::SyntaxKind::DPI_EXPORT,
        slang::SyntaxKind::DPI_IMPORT => syntax::SyntaxKind::DPI_IMPORT,
        slang::SyntaxKind::DRIVE_STRENGTH => syntax::SyntaxKind::DRIVE_STRENGTH,
        slang::SyntaxKind::EDGE_CONTROL_SPECIFIER => syntax::SyntaxKind::EDGE_CONTROL_SPECIFIER,
        slang::SyntaxKind::EDGE_DESCRIPTOR => syntax::SyntaxKind::EDGE_DESCRIPTOR,
        slang::SyntaxKind::EDGE_SENSITIVE_PATH_SUFFIX => {
            syntax::SyntaxKind::EDGE_SENSITIVE_PATH_SUFFIX
        }
        slang::SyntaxKind::ELAB_SYSTEM_TASK => syntax::SyntaxKind::ELAB_SYSTEM_TASK,
        slang::SyntaxKind::ELEMENT_SELECT => syntax::SyntaxKind::ELEMENT_SELECT,
        slang::SyntaxKind::ELEMENT_SELECT_EXPRESSION => {
            syntax::SyntaxKind::ELEMENT_SELECT_EXPRESSION
        }
        slang::SyntaxKind::ELS_IF_DIRECTIVE => syntax::SyntaxKind::ELS_IF_DIRECTIVE,
        slang::SyntaxKind::ELSE_CLAUSE => syntax::SyntaxKind::ELSE_CLAUSE,
        slang::SyntaxKind::ELSE_CONSTRAINT_CLAUSE => syntax::SyntaxKind::ELSE_CONSTRAINT_CLAUSE,
        slang::SyntaxKind::ELSE_DIRECTIVE => syntax::SyntaxKind::ELSE_DIRECTIVE,
        slang::SyntaxKind::ELSE_PROPERTY_CLAUSE => syntax::SyntaxKind::ELSE_PROPERTY_CLAUSE,
        slang::SyntaxKind::EMPTY_ARGUMENT => syntax::SyntaxKind::EMPTY_ARGUMENT,
        slang::SyntaxKind::EMPTY_IDENTIFIER_NAME => syntax::SyntaxKind::EMPTY_IDENTIFIER_NAME,
        slang::SyntaxKind::EMPTY_MEMBER => syntax::SyntaxKind::EMPTY_MEMBER,
        slang::SyntaxKind::EMPTY_NON_ANSI_PORT => syntax::SyntaxKind::EMPTY_NON_ANSI_PORT,
        slang::SyntaxKind::EMPTY_PORT_CONNECTION => syntax::SyntaxKind::EMPTY_PORT_CONNECTION,
        slang::SyntaxKind::EMPTY_QUEUE_EXPRESSION => syntax::SyntaxKind::EMPTY_QUEUE_EXPRESSION,
        slang::SyntaxKind::EMPTY_STATEMENT => syntax::SyntaxKind::EMPTY_STATEMENT,
        slang::SyntaxKind::EMPTY_TIMING_CHECK_ARG => syntax::SyntaxKind::EMPTY_TIMING_CHECK_ARG,
        slang::SyntaxKind::END_CELL_DEFINE_DIRECTIVE => {
            syntax::SyntaxKind::END_CELL_DEFINE_DIRECTIVE
        }
        slang::SyntaxKind::END_IF_DIRECTIVE => syntax::SyntaxKind::END_IF_DIRECTIVE,
        slang::SyntaxKind::END_KEYWORDS_DIRECTIVE => syntax::SyntaxKind::END_KEYWORDS_DIRECTIVE,
        slang::SyntaxKind::END_PROTECT_DIRECTIVE => syntax::SyntaxKind::END_PROTECT_DIRECTIVE,
        slang::SyntaxKind::END_PROTECTED_DIRECTIVE => syntax::SyntaxKind::END_PROTECTED_DIRECTIVE,
        slang::SyntaxKind::ENUM_TYPE => syntax::SyntaxKind::ENUM_TYPE,
        slang::SyntaxKind::EQUALITY_EXPRESSION => syntax::SyntaxKind::EQUALITY_EXPRESSION,
        slang::SyntaxKind::EQUALS_ASSERTION_ARG_CLAUSE => {
            syntax::SyntaxKind::EQUALS_ASSERTION_ARG_CLAUSE
        }
        slang::SyntaxKind::EQUALS_TYPE_CLAUSE => syntax::SyntaxKind::EQUALS_TYPE_CLAUSE,
        slang::SyntaxKind::EQUALS_VALUE_CLAUSE => syntax::SyntaxKind::EQUALS_VALUE_CLAUSE,
        slang::SyntaxKind::EVENT_CONTROL => syntax::SyntaxKind::EVENT_CONTROL,
        slang::SyntaxKind::EVENT_CONTROL_WITH_EXPRESSION => {
            syntax::SyntaxKind::EVENT_CONTROL_WITH_EXPRESSION
        }
        slang::SyntaxKind::EVENT_TYPE => syntax::SyntaxKind::EVENT_TYPE,
        slang::SyntaxKind::EXPECT_PROPERTY_STATEMENT => {
            syntax::SyntaxKind::EXPECT_PROPERTY_STATEMENT
        }
        slang::SyntaxKind::EXPLICIT_ANSI_PORT => syntax::SyntaxKind::EXPLICIT_ANSI_PORT,
        slang::SyntaxKind::EXPLICIT_NON_ANSI_PORT => syntax::SyntaxKind::EXPLICIT_NON_ANSI_PORT,
        slang::SyntaxKind::EXPRESSION_CONSTRAINT => syntax::SyntaxKind::EXPRESSION_CONSTRAINT,
        slang::SyntaxKind::EXPRESSION_COVERAGE_BIN_INITIALIZER => {
            syntax::SyntaxKind::EXPRESSION_COVERAGE_BIN_INITIALIZER
        }
        slang::SyntaxKind::EXPRESSION_OR_DIST => syntax::SyntaxKind::EXPRESSION_OR_DIST,
        slang::SyntaxKind::EXPRESSION_PATTERN => syntax::SyntaxKind::EXPRESSION_PATTERN,
        slang::SyntaxKind::EXPRESSION_STATEMENT => syntax::SyntaxKind::EXPRESSION_STATEMENT,
        slang::SyntaxKind::EXPRESSION_TIMING_CHECK_ARG => {
            syntax::SyntaxKind::EXPRESSION_TIMING_CHECK_ARG
        }
        slang::SyntaxKind::EXTENDS_CLAUSE => syntax::SyntaxKind::EXTENDS_CLAUSE,
        slang::SyntaxKind::EXTERN_INTERFACE_METHOD => syntax::SyntaxKind::EXTERN_INTERFACE_METHOD,
        slang::SyntaxKind::EXTERN_MODULE_DECL => syntax::SyntaxKind::EXTERN_MODULE_DECL,
        slang::SyntaxKind::EXTERN_UDP_DECL => syntax::SyntaxKind::EXTERN_UDP_DECL,
        slang::SyntaxKind::FILE_PATH_SPEC => syntax::SyntaxKind::FILE_PATH_SPEC,
        slang::SyntaxKind::FINAL_BLOCK => syntax::SyntaxKind::FINAL_BLOCK,
        slang::SyntaxKind::FIRST_MATCH_SEQUENCE_EXPR => {
            syntax::SyntaxKind::FIRST_MATCH_SEQUENCE_EXPR
        }
        slang::SyntaxKind::FOLLOWED_BY_PROPERTY_EXPR => {
            syntax::SyntaxKind::FOLLOWED_BY_PROPERTY_EXPR
        }
        slang::SyntaxKind::FOR_LOOP_STATEMENT => syntax::SyntaxKind::FOR_LOOP_STATEMENT,
        slang::SyntaxKind::FOR_VARIABLE_DECLARATION => syntax::SyntaxKind::FOR_VARIABLE_DECLARATION,
        slang::SyntaxKind::FOREACH_LOOP_LIST => syntax::SyntaxKind::FOREACH_LOOP_LIST,
        slang::SyntaxKind::FOREACH_LOOP_STATEMENT => syntax::SyntaxKind::FOREACH_LOOP_STATEMENT,
        slang::SyntaxKind::FOREVER_STATEMENT => syntax::SyntaxKind::FOREVER_STATEMENT,
        slang::SyntaxKind::FORWARD_TYPE_RESTRICTION => syntax::SyntaxKind::FORWARD_TYPE_RESTRICTION,
        slang::SyntaxKind::FORWARD_TYPEDEF_DECLARATION => {
            syntax::SyntaxKind::FORWARD_TYPEDEF_DECLARATION
        }
        slang::SyntaxKind::FUNCTION_DECLARATION => syntax::SyntaxKind::FUNCTION_DECLARATION,
        slang::SyntaxKind::FUNCTION_PORT => syntax::SyntaxKind::FUNCTION_PORT,
        slang::SyntaxKind::FUNCTION_PORT_LIST => syntax::SyntaxKind::FUNCTION_PORT_LIST,
        slang::SyntaxKind::FUNCTION_PROTOTYPE => syntax::SyntaxKind::FUNCTION_PROTOTYPE,
        slang::SyntaxKind::GENERATE_BLOCK => syntax::SyntaxKind::GENERATE_BLOCK,
        slang::SyntaxKind::GENERATE_REGION => syntax::SyntaxKind::GENERATE_REGION,
        slang::SyntaxKind::GENVAR_DECLARATION => syntax::SyntaxKind::GENVAR_DECLARATION,
        slang::SyntaxKind::GREATER_THAN_EQUAL_EXPRESSION => {
            syntax::SyntaxKind::GREATER_THAN_EQUAL_EXPRESSION
        }
        slang::SyntaxKind::GREATER_THAN_EXPRESSION => syntax::SyntaxKind::GREATER_THAN_EXPRESSION,
        slang::SyntaxKind::HIERARCHICAL_INSTANCE => syntax::SyntaxKind::HIERARCHICAL_INSTANCE,
        slang::SyntaxKind::HIERARCHY_INSTANTIATION => syntax::SyntaxKind::HIERARCHY_INSTANTIATION,
        slang::SyntaxKind::ID_WITH_EXPR_COVERAGE_BIN_INITIALIZER => {
            syntax::SyntaxKind::ID_WITH_EXPR_COVERAGE_BIN_INITIALIZER
        }
        slang::SyntaxKind::IDENTIFIER_NAME => syntax::SyntaxKind::IDENTIFIER_NAME,
        slang::SyntaxKind::IDENTIFIER_SELECT_NAME => syntax::SyntaxKind::IDENTIFIER_SELECT_NAME,
        slang::SyntaxKind::IF_DEF_DIRECTIVE => syntax::SyntaxKind::IF_DEF_DIRECTIVE,
        slang::SyntaxKind::IF_GENERATE => syntax::SyntaxKind::IF_GENERATE,
        slang::SyntaxKind::IF_N_DEF_DIRECTIVE => syntax::SyntaxKind::IF_N_DEF_DIRECTIVE,
        slang::SyntaxKind::IF_NONE_PATH_DECLARATION => syntax::SyntaxKind::IF_NONE_PATH_DECLARATION,
        slang::SyntaxKind::IFF_EVENT_CLAUSE => syntax::SyntaxKind::IFF_EVENT_CLAUSE,
        slang::SyntaxKind::IFF_PROPERTY_EXPR => syntax::SyntaxKind::IFF_PROPERTY_EXPR,
        slang::SyntaxKind::IMMEDIATE_ASSERT_STATEMENT => {
            syntax::SyntaxKind::IMMEDIATE_ASSERT_STATEMENT
        }
        slang::SyntaxKind::IMMEDIATE_ASSERTION_MEMBER => {
            syntax::SyntaxKind::IMMEDIATE_ASSERTION_MEMBER
        }
        slang::SyntaxKind::IMMEDIATE_ASSUME_STATEMENT => {
            syntax::SyntaxKind::IMMEDIATE_ASSUME_STATEMENT
        }
        slang::SyntaxKind::IMMEDIATE_COVER_STATEMENT => {
            syntax::SyntaxKind::IMMEDIATE_COVER_STATEMENT
        }
        slang::SyntaxKind::IMPLEMENTS_CLAUSE => syntax::SyntaxKind::IMPLEMENTS_CLAUSE,
        slang::SyntaxKind::IMPLICATION_CONSTRAINT => syntax::SyntaxKind::IMPLICATION_CONSTRAINT,
        slang::SyntaxKind::IMPLICATION_PROPERTY_EXPR => {
            syntax::SyntaxKind::IMPLICATION_PROPERTY_EXPR
        }
        slang::SyntaxKind::IMPLICIT_ANSI_PORT => syntax::SyntaxKind::IMPLICIT_ANSI_PORT,
        slang::SyntaxKind::IMPLICIT_EVENT_CONTROL => syntax::SyntaxKind::IMPLICIT_EVENT_CONTROL,
        slang::SyntaxKind::IMPLICIT_NON_ANSI_PORT => syntax::SyntaxKind::IMPLICIT_NON_ANSI_PORT,
        slang::SyntaxKind::IMPLICIT_TYPE => syntax::SyntaxKind::IMPLICIT_TYPE,
        slang::SyntaxKind::IMPLIES_PROPERTY_EXPR => syntax::SyntaxKind::IMPLIES_PROPERTY_EXPR,
        slang::SyntaxKind::INCLUDE_DIRECTIVE => syntax::SyntaxKind::INCLUDE_DIRECTIVE,
        slang::SyntaxKind::INEQUALITY_EXPRESSION => syntax::SyntaxKind::INEQUALITY_EXPRESSION,
        slang::SyntaxKind::INITIAL_BLOCK => syntax::SyntaxKind::INITIAL_BLOCK,
        slang::SyntaxKind::INSIDE_EXPRESSION => syntax::SyntaxKind::INSIDE_EXPRESSION,
        slang::SyntaxKind::INSTANCE_CONFIG_RULE => syntax::SyntaxKind::INSTANCE_CONFIG_RULE,
        slang::SyntaxKind::INSTANCE_NAME => syntax::SyntaxKind::INSTANCE_NAME,
        slang::SyntaxKind::INT_TYPE => syntax::SyntaxKind::INT_TYPE,
        slang::SyntaxKind::INTEGER_LITERAL_EXPRESSION => {
            syntax::SyntaxKind::INTEGER_LITERAL_EXPRESSION
        }
        slang::SyntaxKind::INTEGER_TYPE => syntax::SyntaxKind::INTEGER_TYPE,
        slang::SyntaxKind::INTEGER_VECTOR_EXPRESSION => {
            syntax::SyntaxKind::INTEGER_VECTOR_EXPRESSION
        }
        slang::SyntaxKind::INTERFACE_DECLARATION => syntax::SyntaxKind::INTERFACE_DECLARATION,
        slang::SyntaxKind::INTERFACE_HEADER => syntax::SyntaxKind::INTERFACE_HEADER,
        slang::SyntaxKind::INTERFACE_PORT_HEADER => syntax::SyntaxKind::INTERFACE_PORT_HEADER,
        slang::SyntaxKind::INTERSECT_CLAUSE => syntax::SyntaxKind::INTERSECT_CLAUSE,
        slang::SyntaxKind::INTERSECT_SEQUENCE_EXPR => syntax::SyntaxKind::INTERSECT_SEQUENCE_EXPR,
        slang::SyntaxKind::INVOCATION_EXPRESSION => syntax::SyntaxKind::INVOCATION_EXPRESSION,
        slang::SyntaxKind::JUMP_STATEMENT => syntax::SyntaxKind::JUMP_STATEMENT,
        slang::SyntaxKind::LESS_THAN_EQUAL_EXPRESSION => {
            syntax::SyntaxKind::LESS_THAN_EQUAL_EXPRESSION
        }
        slang::SyntaxKind::LESS_THAN_EXPRESSION => syntax::SyntaxKind::LESS_THAN_EXPRESSION,
        slang::SyntaxKind::LET_DECLARATION => syntax::SyntaxKind::LET_DECLARATION,
        slang::SyntaxKind::LIBRARY_DECLARATION => syntax::SyntaxKind::LIBRARY_DECLARATION,
        slang::SyntaxKind::LIBRARY_INC_DIR_CLAUSE => syntax::SyntaxKind::LIBRARY_INC_DIR_CLAUSE,
        slang::SyntaxKind::LIBRARY_INCLUDE_STATEMENT => {
            syntax::SyntaxKind::LIBRARY_INCLUDE_STATEMENT
        }
        slang::SyntaxKind::LIBRARY_MAP => syntax::SyntaxKind::LIBRARY_MAP,
        slang::SyntaxKind::LINE_DIRECTIVE => syntax::SyntaxKind::LINE_DIRECTIVE,
        slang::SyntaxKind::LOCAL_SCOPE => syntax::SyntaxKind::LOCAL_SCOPE,
        slang::SyntaxKind::LOCAL_VARIABLE_DECLARATION => {
            syntax::SyntaxKind::LOCAL_VARIABLE_DECLARATION
        }
        slang::SyntaxKind::LOGIC_TYPE => syntax::SyntaxKind::LOGIC_TYPE,
        slang::SyntaxKind::LOGICAL_AND_EXPRESSION => syntax::SyntaxKind::LOGICAL_AND_EXPRESSION,
        slang::SyntaxKind::LOGICAL_EQUIVALENCE_EXPRESSION => {
            syntax::SyntaxKind::LOGICAL_EQUIVALENCE_EXPRESSION
        }
        slang::SyntaxKind::LOGICAL_IMPLICATION_EXPRESSION => {
            syntax::SyntaxKind::LOGICAL_IMPLICATION_EXPRESSION
        }
        slang::SyntaxKind::LOGICAL_LEFT_SHIFT_ASSIGNMENT_EXPRESSION => {
            syntax::SyntaxKind::LOGICAL_LEFT_SHIFT_ASSIGNMENT_EXPRESSION
        }
        slang::SyntaxKind::LOGICAL_OR_EXPRESSION => syntax::SyntaxKind::LOGICAL_OR_EXPRESSION,
        slang::SyntaxKind::LOGICAL_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION => {
            syntax::SyntaxKind::LOGICAL_RIGHT_SHIFT_ASSIGNMENT_EXPRESSION
        }
        slang::SyntaxKind::LOGICAL_SHIFT_LEFT_EXPRESSION => {
            syntax::SyntaxKind::LOGICAL_SHIFT_LEFT_EXPRESSION
        }
        slang::SyntaxKind::LOGICAL_SHIFT_RIGHT_EXPRESSION => {
            syntax::SyntaxKind::LOGICAL_SHIFT_RIGHT_EXPRESSION
        }
        slang::SyntaxKind::LONG_INT_TYPE => syntax::SyntaxKind::LONG_INT_TYPE,
        slang::SyntaxKind::LOOP_CONSTRAINT => syntax::SyntaxKind::LOOP_CONSTRAINT,
        slang::SyntaxKind::LOOP_GENERATE => syntax::SyntaxKind::LOOP_GENERATE,
        slang::SyntaxKind::LOOP_STATEMENT => syntax::SyntaxKind::LOOP_STATEMENT,
        slang::SyntaxKind::MACRO_ACTUAL_ARGUMENT => syntax::SyntaxKind::MACRO_ACTUAL_ARGUMENT,
        slang::SyntaxKind::MACRO_ACTUAL_ARGUMENT_LIST => {
            syntax::SyntaxKind::MACRO_ACTUAL_ARGUMENT_LIST
        }
        slang::SyntaxKind::MACRO_ARGUMENT_DEFAULT => syntax::SyntaxKind::MACRO_ARGUMENT_DEFAULT,
        slang::SyntaxKind::MACRO_FORMAL_ARGUMENT => syntax::SyntaxKind::MACRO_FORMAL_ARGUMENT,
        slang::SyntaxKind::MACRO_FORMAL_ARGUMENT_LIST => {
            syntax::SyntaxKind::MACRO_FORMAL_ARGUMENT_LIST
        }
        slang::SyntaxKind::MACRO_USAGE => syntax::SyntaxKind::MACRO_USAGE,
        slang::SyntaxKind::MATCHES_CLAUSE => syntax::SyntaxKind::MATCHES_CLAUSE,
        slang::SyntaxKind::MEMBER_ACCESS_EXPRESSION => syntax::SyntaxKind::MEMBER_ACCESS_EXPRESSION,
        slang::SyntaxKind::MIN_TYP_MAX_EXPRESSION => syntax::SyntaxKind::MIN_TYP_MAX_EXPRESSION,
        slang::SyntaxKind::MOD_ASSIGNMENT_EXPRESSION => {
            syntax::SyntaxKind::MOD_ASSIGNMENT_EXPRESSION
        }
        slang::SyntaxKind::MOD_EXPRESSION => syntax::SyntaxKind::MOD_EXPRESSION,
        slang::SyntaxKind::MODPORT_CLOCKING_PORT => syntax::SyntaxKind::MODPORT_CLOCKING_PORT,
        slang::SyntaxKind::MODPORT_DECLARATION => syntax::SyntaxKind::MODPORT_DECLARATION,
        slang::SyntaxKind::MODPORT_EXPLICIT_PORT => syntax::SyntaxKind::MODPORT_EXPLICIT_PORT,
        slang::SyntaxKind::MODPORT_ITEM => syntax::SyntaxKind::MODPORT_ITEM,
        slang::SyntaxKind::MODPORT_NAMED_PORT => syntax::SyntaxKind::MODPORT_NAMED_PORT,
        slang::SyntaxKind::MODPORT_SIMPLE_PORT_LIST => syntax::SyntaxKind::MODPORT_SIMPLE_PORT_LIST,
        slang::SyntaxKind::MODPORT_SUBROUTINE_PORT => syntax::SyntaxKind::MODPORT_SUBROUTINE_PORT,
        slang::SyntaxKind::MODPORT_SUBROUTINE_PORT_LIST => {
            syntax::SyntaxKind::MODPORT_SUBROUTINE_PORT_LIST
        }
        slang::SyntaxKind::MODULE_DECLARATION => syntax::SyntaxKind::MODULE_DECLARATION,
        slang::SyntaxKind::MODULE_HEADER => syntax::SyntaxKind::MODULE_HEADER,
        slang::SyntaxKind::MULTIPLE_CONCATENATION_EXPRESSION => {
            syntax::SyntaxKind::MULTIPLE_CONCATENATION_EXPRESSION
        }
        slang::SyntaxKind::MULTIPLY_ASSIGNMENT_EXPRESSION => {
            syntax::SyntaxKind::MULTIPLY_ASSIGNMENT_EXPRESSION
        }
        slang::SyntaxKind::MULTIPLY_EXPRESSION => syntax::SyntaxKind::MULTIPLY_EXPRESSION,
        slang::SyntaxKind::NAME_VALUE_PRAGMA_EXPRESSION => {
            syntax::SyntaxKind::NAME_VALUE_PRAGMA_EXPRESSION
        }
        slang::SyntaxKind::NAMED_ARGUMENT => syntax::SyntaxKind::NAMED_ARGUMENT,
        slang::SyntaxKind::NAMED_BLOCK_CLAUSE => syntax::SyntaxKind::NAMED_BLOCK_CLAUSE,
        slang::SyntaxKind::NAMED_CONDITIONAL_DIRECTIVE_EXPRESSION => {
            syntax::SyntaxKind::NAMED_CONDITIONAL_DIRECTIVE_EXPRESSION
        }
        slang::SyntaxKind::NAMED_LABEL => syntax::SyntaxKind::NAMED_LABEL,
        slang::SyntaxKind::NAMED_PARAM_ASSIGNMENT => syntax::SyntaxKind::NAMED_PARAM_ASSIGNMENT,
        slang::SyntaxKind::NAMED_PORT_CONNECTION => syntax::SyntaxKind::NAMED_PORT_CONNECTION,
        slang::SyntaxKind::NAMED_STRUCTURE_PATTERN_MEMBER => {
            syntax::SyntaxKind::NAMED_STRUCTURE_PATTERN_MEMBER
        }
        slang::SyntaxKind::NAMED_TYPE => syntax::SyntaxKind::NAMED_TYPE,
        slang::SyntaxKind::NET_ALIAS => syntax::SyntaxKind::NET_ALIAS,
        slang::SyntaxKind::NET_DECLARATION => syntax::SyntaxKind::NET_DECLARATION,
        slang::SyntaxKind::NET_PORT_HEADER => syntax::SyntaxKind::NET_PORT_HEADER,
        slang::SyntaxKind::NET_TYPE_DECLARATION => syntax::SyntaxKind::NET_TYPE_DECLARATION,
        slang::SyntaxKind::NEW_ARRAY_EXPRESSION => syntax::SyntaxKind::NEW_ARRAY_EXPRESSION,
        slang::SyntaxKind::NEW_CLASS_EXPRESSION => syntax::SyntaxKind::NEW_CLASS_EXPRESSION,
        slang::SyntaxKind::NO_UNCONNECTED_DRIVE_DIRECTIVE => {
            syntax::SyntaxKind::NO_UNCONNECTED_DRIVE_DIRECTIVE
        }
        slang::SyntaxKind::NON_ANSI_PORT_LIST => syntax::SyntaxKind::NON_ANSI_PORT_LIST,
        slang::SyntaxKind::NON_ANSI_UDP_PORT_LIST => syntax::SyntaxKind::NON_ANSI_UDP_PORT_LIST,
        slang::SyntaxKind::NONBLOCKING_ASSIGNMENT_EXPRESSION => {
            syntax::SyntaxKind::NONBLOCKING_ASSIGNMENT_EXPRESSION
        }
        slang::SyntaxKind::NONBLOCKING_EVENT_TRIGGER_STATEMENT => {
            syntax::SyntaxKind::NONBLOCKING_EVENT_TRIGGER_STATEMENT
        }
        slang::SyntaxKind::NULL_LITERAL_EXPRESSION => syntax::SyntaxKind::NULL_LITERAL_EXPRESSION,
        slang::SyntaxKind::NUMBER_PRAGMA_EXPRESSION => syntax::SyntaxKind::NUMBER_PRAGMA_EXPRESSION,
        slang::SyntaxKind::ONE_STEP_DELAY => syntax::SyntaxKind::ONE_STEP_DELAY,
        slang::SyntaxKind::OR_ASSIGNMENT_EXPRESSION => syntax::SyntaxKind::OR_ASSIGNMENT_EXPRESSION,
        slang::SyntaxKind::OR_PROPERTY_EXPR => syntax::SyntaxKind::OR_PROPERTY_EXPR,
        slang::SyntaxKind::OR_SEQUENCE_EXPR => syntax::SyntaxKind::OR_SEQUENCE_EXPR,
        slang::SyntaxKind::ORDERED_ARGUMENT => syntax::SyntaxKind::ORDERED_ARGUMENT,
        slang::SyntaxKind::ORDERED_PARAM_ASSIGNMENT => syntax::SyntaxKind::ORDERED_PARAM_ASSIGNMENT,
        slang::SyntaxKind::ORDERED_PORT_CONNECTION => syntax::SyntaxKind::ORDERED_PORT_CONNECTION,
        slang::SyntaxKind::ORDERED_STRUCTURE_PATTERN_MEMBER => {
            syntax::SyntaxKind::ORDERED_STRUCTURE_PATTERN_MEMBER
        }
        slang::SyntaxKind::PACKAGE_DECLARATION => syntax::SyntaxKind::PACKAGE_DECLARATION,
        slang::SyntaxKind::PACKAGE_EXPORT_ALL_DECLARATION => {
            syntax::SyntaxKind::PACKAGE_EXPORT_ALL_DECLARATION
        }
        slang::SyntaxKind::PACKAGE_EXPORT_DECLARATION => {
            syntax::SyntaxKind::PACKAGE_EXPORT_DECLARATION
        }
        slang::SyntaxKind::PACKAGE_HEADER => syntax::SyntaxKind::PACKAGE_HEADER,
        slang::SyntaxKind::PACKAGE_IMPORT_DECLARATION => {
            syntax::SyntaxKind::PACKAGE_IMPORT_DECLARATION
        }
        slang::SyntaxKind::PACKAGE_IMPORT_ITEM => syntax::SyntaxKind::PACKAGE_IMPORT_ITEM,
        slang::SyntaxKind::PARALLEL_BLOCK_STATEMENT => syntax::SyntaxKind::PARALLEL_BLOCK_STATEMENT,
        slang::SyntaxKind::PARAMETER_DECLARATION => syntax::SyntaxKind::PARAMETER_DECLARATION,
        slang::SyntaxKind::PARAMETER_DECLARATION_STATEMENT => {
            syntax::SyntaxKind::PARAMETER_DECLARATION_STATEMENT
        }
        slang::SyntaxKind::PARAMETER_PORT_LIST => syntax::SyntaxKind::PARAMETER_PORT_LIST,
        slang::SyntaxKind::PARAMETER_VALUE_ASSIGNMENT => {
            syntax::SyntaxKind::PARAMETER_VALUE_ASSIGNMENT
        }
        slang::SyntaxKind::PAREN_EXPRESSION_LIST => syntax::SyntaxKind::PAREN_EXPRESSION_LIST,
        slang::SyntaxKind::PAREN_PRAGMA_EXPRESSION => syntax::SyntaxKind::PAREN_PRAGMA_EXPRESSION,
        slang::SyntaxKind::PARENTHESIZED_BINS_SELECT_EXPR => {
            syntax::SyntaxKind::PARENTHESIZED_BINS_SELECT_EXPR
        }
        slang::SyntaxKind::PARENTHESIZED_CONDITIONAL_DIRECTIVE_EXPRESSION => {
            syntax::SyntaxKind::PARENTHESIZED_CONDITIONAL_DIRECTIVE_EXPRESSION
        }
        slang::SyntaxKind::PARENTHESIZED_EVENT_EXPRESSION => {
            syntax::SyntaxKind::PARENTHESIZED_EVENT_EXPRESSION
        }
        slang::SyntaxKind::PARENTHESIZED_EXPRESSION => syntax::SyntaxKind::PARENTHESIZED_EXPRESSION,
        slang::SyntaxKind::PARENTHESIZED_PATTERN => syntax::SyntaxKind::PARENTHESIZED_PATTERN,
        slang::SyntaxKind::PARENTHESIZED_PROPERTY_EXPR => {
            syntax::SyntaxKind::PARENTHESIZED_PROPERTY_EXPR
        }
        slang::SyntaxKind::PARENTHESIZED_SEQUENCE_EXPR => {
            syntax::SyntaxKind::PARENTHESIZED_SEQUENCE_EXPR
        }
        slang::SyntaxKind::PATH_DECLARATION => syntax::SyntaxKind::PATH_DECLARATION,
        slang::SyntaxKind::PATH_DESCRIPTION => syntax::SyntaxKind::PATH_DESCRIPTION,
        slang::SyntaxKind::PATTERN_CASE_ITEM => syntax::SyntaxKind::PATTERN_CASE_ITEM,
        slang::SyntaxKind::PORT_CONCATENATION => syntax::SyntaxKind::PORT_CONCATENATION,
        slang::SyntaxKind::PORT_DECLARATION => syntax::SyntaxKind::PORT_DECLARATION,
        slang::SyntaxKind::PORT_REFERENCE => syntax::SyntaxKind::PORT_REFERENCE,
        slang::SyntaxKind::POSTDECREMENT_EXPRESSION => syntax::SyntaxKind::POSTDECREMENT_EXPRESSION,
        slang::SyntaxKind::POSTINCREMENT_EXPRESSION => syntax::SyntaxKind::POSTINCREMENT_EXPRESSION,
        slang::SyntaxKind::POWER_EXPRESSION => syntax::SyntaxKind::POWER_EXPRESSION,
        slang::SyntaxKind::PRAGMA_DIRECTIVE => syntax::SyntaxKind::PRAGMA_DIRECTIVE,
        slang::SyntaxKind::PRIMARY_BLOCK_EVENT_EXPRESSION => {
            syntax::SyntaxKind::PRIMARY_BLOCK_EVENT_EXPRESSION
        }
        slang::SyntaxKind::PRIMITIVE_INSTANTIATION => syntax::SyntaxKind::PRIMITIVE_INSTANTIATION,
        slang::SyntaxKind::PROCEDURAL_ASSIGN_STATEMENT => {
            syntax::SyntaxKind::PROCEDURAL_ASSIGN_STATEMENT
        }
        slang::SyntaxKind::PROCEDURAL_DEASSIGN_STATEMENT => {
            syntax::SyntaxKind::PROCEDURAL_DEASSIGN_STATEMENT
        }
        slang::SyntaxKind::PROCEDURAL_FORCE_STATEMENT => {
            syntax::SyntaxKind::PROCEDURAL_FORCE_STATEMENT
        }
        slang::SyntaxKind::PROCEDURAL_RELEASE_STATEMENT => {
            syntax::SyntaxKind::PROCEDURAL_RELEASE_STATEMENT
        }
        slang::SyntaxKind::PRODUCTION => syntax::SyntaxKind::PRODUCTION,
        slang::SyntaxKind::PROGRAM_DECLARATION => syntax::SyntaxKind::PROGRAM_DECLARATION,
        slang::SyntaxKind::PROGRAM_HEADER => syntax::SyntaxKind::PROGRAM_HEADER,
        slang::SyntaxKind::PROPERTY_DECLARATION => syntax::SyntaxKind::PROPERTY_DECLARATION,
        slang::SyntaxKind::PROPERTY_SPEC => syntax::SyntaxKind::PROPERTY_SPEC,
        slang::SyntaxKind::PROPERTY_TYPE => syntax::SyntaxKind::PROPERTY_TYPE,
        slang::SyntaxKind::PROTECT_DIRECTIVE => syntax::SyntaxKind::PROTECT_DIRECTIVE,
        slang::SyntaxKind::PROTECTED_DIRECTIVE => syntax::SyntaxKind::PROTECTED_DIRECTIVE,
        slang::SyntaxKind::PULL_STRENGTH => syntax::SyntaxKind::PULL_STRENGTH,
        slang::SyntaxKind::PULSE_STYLE_DECLARATION => syntax::SyntaxKind::PULSE_STYLE_DECLARATION,
        slang::SyntaxKind::QUEUE_DIMENSION_SPECIFIER => {
            syntax::SyntaxKind::QUEUE_DIMENSION_SPECIFIER
        }
        slang::SyntaxKind::RAND_CASE_ITEM => syntax::SyntaxKind::RAND_CASE_ITEM,
        slang::SyntaxKind::RAND_CASE_STATEMENT => syntax::SyntaxKind::RAND_CASE_STATEMENT,
        slang::SyntaxKind::RAND_JOIN_CLAUSE => syntax::SyntaxKind::RAND_JOIN_CLAUSE,
        slang::SyntaxKind::RAND_SEQUENCE_STATEMENT => syntax::SyntaxKind::RAND_SEQUENCE_STATEMENT,
        slang::SyntaxKind::RANGE_COVERAGE_BIN_INITIALIZER => {
            syntax::SyntaxKind::RANGE_COVERAGE_BIN_INITIALIZER
        }
        slang::SyntaxKind::RANGE_DIMENSION_SPECIFIER => {
            syntax::SyntaxKind::RANGE_DIMENSION_SPECIFIER
        }
        slang::SyntaxKind::RANGE_LIST => syntax::SyntaxKind::RANGE_LIST,
        slang::SyntaxKind::REAL_LITERAL_EXPRESSION => syntax::SyntaxKind::REAL_LITERAL_EXPRESSION,
        slang::SyntaxKind::REAL_TIME_TYPE => syntax::SyntaxKind::REAL_TIME_TYPE,
        slang::SyntaxKind::REAL_TYPE => syntax::SyntaxKind::REAL_TYPE,
        slang::SyntaxKind::REG_TYPE => syntax::SyntaxKind::REG_TYPE,
        slang::SyntaxKind::REPEATED_EVENT_CONTROL => syntax::SyntaxKind::REPEATED_EVENT_CONTROL,
        slang::SyntaxKind::REPLICATED_ASSIGNMENT_PATTERN => {
            syntax::SyntaxKind::REPLICATED_ASSIGNMENT_PATTERN
        }
        slang::SyntaxKind::RESET_ALL_DIRECTIVE => syntax::SyntaxKind::RESET_ALL_DIRECTIVE,
        slang::SyntaxKind::RESTRICT_PROPERTY_STATEMENT => {
            syntax::SyntaxKind::RESTRICT_PROPERTY_STATEMENT
        }
        slang::SyntaxKind::RETURN_STATEMENT => syntax::SyntaxKind::RETURN_STATEMENT,
        slang::SyntaxKind::ROOT_SCOPE => syntax::SyntaxKind::ROOT_SCOPE,
        slang::SyntaxKind::RS_CASE => syntax::SyntaxKind::RS_CASE,
        slang::SyntaxKind::RS_CODE_BLOCK => syntax::SyntaxKind::RS_CODE_BLOCK,
        slang::SyntaxKind::RS_ELSE_CLAUSE => syntax::SyntaxKind::RS_ELSE_CLAUSE,
        slang::SyntaxKind::RS_IF_ELSE => syntax::SyntaxKind::RS_IF_ELSE,
        slang::SyntaxKind::RS_PROD_ITEM => syntax::SyntaxKind::RS_PROD_ITEM,
        slang::SyntaxKind::RS_REPEAT => syntax::SyntaxKind::RS_REPEAT,
        slang::SyntaxKind::RS_RULE => syntax::SyntaxKind::RS_RULE,
        slang::SyntaxKind::RS_WEIGHT_CLAUSE => syntax::SyntaxKind::RS_WEIGHT_CLAUSE,
        slang::SyntaxKind::S_UNTIL_PROPERTY_EXPR => syntax::SyntaxKind::S_UNTIL_PROPERTY_EXPR,
        slang::SyntaxKind::S_UNTIL_WITH_PROPERTY_EXPR => {
            syntax::SyntaxKind::S_UNTIL_WITH_PROPERTY_EXPR
        }
        slang::SyntaxKind::SCOPED_NAME => syntax::SyntaxKind::SCOPED_NAME,
        slang::SyntaxKind::SEPARATED_LIST => syntax::SyntaxKind::SEPARATED_LIST,
        slang::SyntaxKind::SEQUENCE_DECLARATION => syntax::SyntaxKind::SEQUENCE_DECLARATION,
        slang::SyntaxKind::SEQUENCE_MATCH_LIST => syntax::SyntaxKind::SEQUENCE_MATCH_LIST,
        slang::SyntaxKind::SEQUENCE_REPETITION => syntax::SyntaxKind::SEQUENCE_REPETITION,
        slang::SyntaxKind::SEQUENCE_TYPE => syntax::SyntaxKind::SEQUENCE_TYPE,
        slang::SyntaxKind::SEQUENTIAL_BLOCK_STATEMENT => {
            syntax::SyntaxKind::SEQUENTIAL_BLOCK_STATEMENT
        }
        slang::SyntaxKind::SHORT_INT_TYPE => syntax::SyntaxKind::SHORT_INT_TYPE,
        slang::SyntaxKind::SHORT_REAL_TYPE => syntax::SyntaxKind::SHORT_REAL_TYPE,
        slang::SyntaxKind::SIGNAL_EVENT_EXPRESSION => syntax::SyntaxKind::SIGNAL_EVENT_EXPRESSION,
        slang::SyntaxKind::SIGNED_CAST_EXPRESSION => syntax::SyntaxKind::SIGNED_CAST_EXPRESSION,
        slang::SyntaxKind::SIMPLE_ASSIGNMENT_PATTERN => {
            syntax::SyntaxKind::SIMPLE_ASSIGNMENT_PATTERN
        }
        slang::SyntaxKind::SIMPLE_BINS_SELECT_EXPR => syntax::SyntaxKind::SIMPLE_BINS_SELECT_EXPR,
        slang::SyntaxKind::SIMPLE_PATH_SUFFIX => syntax::SyntaxKind::SIMPLE_PATH_SUFFIX,
        slang::SyntaxKind::SIMPLE_PRAGMA_EXPRESSION => syntax::SyntaxKind::SIMPLE_PRAGMA_EXPRESSION,
        slang::SyntaxKind::SIMPLE_PROPERTY_EXPR => syntax::SyntaxKind::SIMPLE_PROPERTY_EXPR,
        slang::SyntaxKind::SIMPLE_RANGE_SELECT => syntax::SyntaxKind::SIMPLE_RANGE_SELECT,
        slang::SyntaxKind::SIMPLE_SEQUENCE_EXPR => syntax::SyntaxKind::SIMPLE_SEQUENCE_EXPR,
        slang::SyntaxKind::SOLVE_BEFORE_CONSTRAINT => syntax::SyntaxKind::SOLVE_BEFORE_CONSTRAINT,
        slang::SyntaxKind::SPECIFY_BLOCK => syntax::SyntaxKind::SPECIFY_BLOCK,
        slang::SyntaxKind::SPECPARAM_DECLARATION => syntax::SyntaxKind::SPECPARAM_DECLARATION,
        slang::SyntaxKind::SPECPARAM_DECLARATOR => syntax::SyntaxKind::SPECPARAM_DECLARATOR,
        slang::SyntaxKind::STANDARD_CASE_ITEM => syntax::SyntaxKind::STANDARD_CASE_ITEM,
        slang::SyntaxKind::STANDARD_PROPERTY_CASE_ITEM => {
            syntax::SyntaxKind::STANDARD_PROPERTY_CASE_ITEM
        }
        slang::SyntaxKind::STANDARD_RS_CASE_ITEM => syntax::SyntaxKind::STANDARD_RS_CASE_ITEM,
        slang::SyntaxKind::STREAM_EXPRESSION => syntax::SyntaxKind::STREAM_EXPRESSION,
        slang::SyntaxKind::STREAM_EXPRESSION_WITH_RANGE => {
            syntax::SyntaxKind::STREAM_EXPRESSION_WITH_RANGE
        }
        slang::SyntaxKind::STREAMING_CONCATENATION_EXPRESSION => {
            syntax::SyntaxKind::STREAMING_CONCATENATION_EXPRESSION
        }
        slang::SyntaxKind::STRING_LITERAL_EXPRESSION => {
            syntax::SyntaxKind::STRING_LITERAL_EXPRESSION
        }
        slang::SyntaxKind::STRING_TYPE => syntax::SyntaxKind::STRING_TYPE,
        slang::SyntaxKind::STRONG_WEAK_PROPERTY_EXPR => {
            syntax::SyntaxKind::STRONG_WEAK_PROPERTY_EXPR
        }
        slang::SyntaxKind::STRUCT_TYPE => syntax::SyntaxKind::STRUCT_TYPE,
        slang::SyntaxKind::STRUCT_UNION_MEMBER => syntax::SyntaxKind::STRUCT_UNION_MEMBER,
        slang::SyntaxKind::STRUCTURE_PATTERN => syntax::SyntaxKind::STRUCTURE_PATTERN,
        slang::SyntaxKind::STRUCTURED_ASSIGNMENT_PATTERN => {
            syntax::SyntaxKind::STRUCTURED_ASSIGNMENT_PATTERN
        }
        slang::SyntaxKind::SUBTRACT_ASSIGNMENT_EXPRESSION => {
            syntax::SyntaxKind::SUBTRACT_ASSIGNMENT_EXPRESSION
        }
        slang::SyntaxKind::SUBTRACT_EXPRESSION => syntax::SyntaxKind::SUBTRACT_EXPRESSION,
        slang::SyntaxKind::SUPER_HANDLE => syntax::SyntaxKind::SUPER_HANDLE,
        slang::SyntaxKind::SUPER_NEW_DEFAULTED_ARGS_EXPRESSION => {
            syntax::SyntaxKind::SUPER_NEW_DEFAULTED_ARGS_EXPRESSION
        }
        slang::SyntaxKind::SYNTAX_LIST => syntax::SyntaxKind::SYNTAX_LIST,
        slang::SyntaxKind::SYSTEM_NAME => syntax::SyntaxKind::SYSTEM_NAME,
        slang::SyntaxKind::SYSTEM_TIMING_CHECK => syntax::SyntaxKind::SYSTEM_TIMING_CHECK,
        slang::SyntaxKind::TAGGED_PATTERN => syntax::SyntaxKind::TAGGED_PATTERN,
        slang::SyntaxKind::TAGGED_UNION_EXPRESSION => syntax::SyntaxKind::TAGGED_UNION_EXPRESSION,
        slang::SyntaxKind::TASK_DECLARATION => syntax::SyntaxKind::TASK_DECLARATION,
        slang::SyntaxKind::THIS_HANDLE => syntax::SyntaxKind::THIS_HANDLE,
        slang::SyntaxKind::THROUGHOUT_SEQUENCE_EXPR => syntax::SyntaxKind::THROUGHOUT_SEQUENCE_EXPR,
        slang::SyntaxKind::TIME_LITERAL_EXPRESSION => syntax::SyntaxKind::TIME_LITERAL_EXPRESSION,
        slang::SyntaxKind::TIME_SCALE_DIRECTIVE => syntax::SyntaxKind::TIME_SCALE_DIRECTIVE,
        slang::SyntaxKind::TIME_TYPE => syntax::SyntaxKind::TIME_TYPE,
        slang::SyntaxKind::TIME_UNITS_DECLARATION => syntax::SyntaxKind::TIME_UNITS_DECLARATION,
        slang::SyntaxKind::TIMING_CHECK_EVENT_ARG => syntax::SyntaxKind::TIMING_CHECK_EVENT_ARG,
        slang::SyntaxKind::TIMING_CHECK_EVENT_CONDITION => {
            syntax::SyntaxKind::TIMING_CHECK_EVENT_CONDITION
        }
        slang::SyntaxKind::TIMING_CONTROL_EXPRESSION => {
            syntax::SyntaxKind::TIMING_CONTROL_EXPRESSION
        }
        slang::SyntaxKind::TIMING_CONTROL_STATEMENT => syntax::SyntaxKind::TIMING_CONTROL_STATEMENT,
        slang::SyntaxKind::TOKEN_LIST => syntax::SyntaxKind::TOKEN_LIST,
        slang::SyntaxKind::TRANS_LIST_COVERAGE_BIN_INITIALIZER => {
            syntax::SyntaxKind::TRANS_LIST_COVERAGE_BIN_INITIALIZER
        }
        slang::SyntaxKind::TRANS_RANGE => syntax::SyntaxKind::TRANS_RANGE,
        slang::SyntaxKind::TRANS_REPEAT_RANGE => syntax::SyntaxKind::TRANS_REPEAT_RANGE,
        slang::SyntaxKind::TRANS_SET => syntax::SyntaxKind::TRANS_SET,
        slang::SyntaxKind::TYPE_ASSIGNMENT => syntax::SyntaxKind::TYPE_ASSIGNMENT,
        slang::SyntaxKind::TYPE_PARAMETER_DECLARATION => {
            syntax::SyntaxKind::TYPE_PARAMETER_DECLARATION
        }
        slang::SyntaxKind::TYPE_REFERENCE => syntax::SyntaxKind::TYPE_REFERENCE,
        slang::SyntaxKind::TYPEDEF_DECLARATION => syntax::SyntaxKind::TYPEDEF_DECLARATION,
        slang::SyntaxKind::UDP_BODY => syntax::SyntaxKind::UDP_BODY,
        slang::SyntaxKind::UDP_DECLARATION => syntax::SyntaxKind::UDP_DECLARATION,
        slang::SyntaxKind::UDP_EDGE_FIELD => syntax::SyntaxKind::UDP_EDGE_FIELD,
        slang::SyntaxKind::UDP_ENTRY => syntax::SyntaxKind::UDP_ENTRY,
        slang::SyntaxKind::UDP_INITIAL_STMT => syntax::SyntaxKind::UDP_INITIAL_STMT,
        slang::SyntaxKind::UDP_INPUT_PORT_DECL => syntax::SyntaxKind::UDP_INPUT_PORT_DECL,
        slang::SyntaxKind::UDP_OUTPUT_PORT_DECL => syntax::SyntaxKind::UDP_OUTPUT_PORT_DECL,
        slang::SyntaxKind::UDP_SIMPLE_FIELD => syntax::SyntaxKind::UDP_SIMPLE_FIELD,
        slang::SyntaxKind::UNARY_BINS_SELECT_EXPR => syntax::SyntaxKind::UNARY_BINS_SELECT_EXPR,
        slang::SyntaxKind::UNARY_BITWISE_AND_EXPRESSION => {
            syntax::SyntaxKind::UNARY_BITWISE_AND_EXPRESSION
        }
        slang::SyntaxKind::UNARY_BITWISE_NAND_EXPRESSION => {
            syntax::SyntaxKind::UNARY_BITWISE_NAND_EXPRESSION
        }
        slang::SyntaxKind::UNARY_BITWISE_NOR_EXPRESSION => {
            syntax::SyntaxKind::UNARY_BITWISE_NOR_EXPRESSION
        }
        slang::SyntaxKind::UNARY_BITWISE_NOT_EXPRESSION => {
            syntax::SyntaxKind::UNARY_BITWISE_NOT_EXPRESSION
        }
        slang::SyntaxKind::UNARY_BITWISE_OR_EXPRESSION => {
            syntax::SyntaxKind::UNARY_BITWISE_OR_EXPRESSION
        }
        slang::SyntaxKind::UNARY_BITWISE_XNOR_EXPRESSION => {
            syntax::SyntaxKind::UNARY_BITWISE_XNOR_EXPRESSION
        }
        slang::SyntaxKind::UNARY_BITWISE_XOR_EXPRESSION => {
            syntax::SyntaxKind::UNARY_BITWISE_XOR_EXPRESSION
        }
        slang::SyntaxKind::UNARY_CONDITIONAL_DIRECTIVE_EXPRESSION => {
            syntax::SyntaxKind::UNARY_CONDITIONAL_DIRECTIVE_EXPRESSION
        }
        slang::SyntaxKind::UNARY_LOGICAL_NOT_EXPRESSION => {
            syntax::SyntaxKind::UNARY_LOGICAL_NOT_EXPRESSION
        }
        slang::SyntaxKind::UNARY_MINUS_EXPRESSION => syntax::SyntaxKind::UNARY_MINUS_EXPRESSION,
        slang::SyntaxKind::UNARY_PLUS_EXPRESSION => syntax::SyntaxKind::UNARY_PLUS_EXPRESSION,
        slang::SyntaxKind::UNARY_PREDECREMENT_EXPRESSION => {
            syntax::SyntaxKind::UNARY_PREDECREMENT_EXPRESSION
        }
        slang::SyntaxKind::UNARY_PREINCREMENT_EXPRESSION => {
            syntax::SyntaxKind::UNARY_PREINCREMENT_EXPRESSION
        }
        slang::SyntaxKind::UNARY_PROPERTY_EXPR => syntax::SyntaxKind::UNARY_PROPERTY_EXPR,
        slang::SyntaxKind::UNARY_SELECT_PROPERTY_EXPR => {
            syntax::SyntaxKind::UNARY_SELECT_PROPERTY_EXPR
        }
        slang::SyntaxKind::UNBASED_UNSIZED_LITERAL_EXPRESSION => {
            syntax::SyntaxKind::UNBASED_UNSIZED_LITERAL_EXPRESSION
        }
        slang::SyntaxKind::UNCONNECTED_DRIVE_DIRECTIVE => {
            syntax::SyntaxKind::UNCONNECTED_DRIVE_DIRECTIVE
        }
        slang::SyntaxKind::UNDEF_DIRECTIVE => syntax::SyntaxKind::UNDEF_DIRECTIVE,
        slang::SyntaxKind::UNDEFINE_ALL_DIRECTIVE => syntax::SyntaxKind::UNDEFINE_ALL_DIRECTIVE,
        slang::SyntaxKind::UNION_TYPE => syntax::SyntaxKind::UNION_TYPE,
        slang::SyntaxKind::UNIQUENESS_CONSTRAINT => syntax::SyntaxKind::UNIQUENESS_CONSTRAINT,
        slang::SyntaxKind::UNIT_SCOPE => syntax::SyntaxKind::UNIT_SCOPE,
        slang::SyntaxKind::UNKNOWN => syntax::SyntaxKind::UNKNOWN,
        slang::SyntaxKind::UNTIL_PROPERTY_EXPR => syntax::SyntaxKind::UNTIL_PROPERTY_EXPR,
        slang::SyntaxKind::UNTIL_WITH_PROPERTY_EXPR => syntax::SyntaxKind::UNTIL_WITH_PROPERTY_EXPR,
        slang::SyntaxKind::UNTYPED => syntax::SyntaxKind::UNTYPED,
        slang::SyntaxKind::USER_DEFINED_NET_DECLARATION => {
            syntax::SyntaxKind::USER_DEFINED_NET_DECLARATION
        }
        slang::SyntaxKind::VALUE_RANGE_EXPRESSION => syntax::SyntaxKind::VALUE_RANGE_EXPRESSION,
        slang::SyntaxKind::VARIABLE_DIMENSION => syntax::SyntaxKind::VARIABLE_DIMENSION,
        slang::SyntaxKind::VARIABLE_PATTERN => syntax::SyntaxKind::VARIABLE_PATTERN,
        slang::SyntaxKind::VARIABLE_PORT_HEADER => syntax::SyntaxKind::VARIABLE_PORT_HEADER,
        slang::SyntaxKind::VIRTUAL_INTERFACE_TYPE => syntax::SyntaxKind::VIRTUAL_INTERFACE_TYPE,
        slang::SyntaxKind::VOID_CASTED_CALL_STATEMENT => {
            syntax::SyntaxKind::VOID_CASTED_CALL_STATEMENT
        }
        slang::SyntaxKind::VOID_TYPE => syntax::SyntaxKind::VOID_TYPE,
        slang::SyntaxKind::WAIT_FORK_STATEMENT => syntax::SyntaxKind::WAIT_FORK_STATEMENT,
        slang::SyntaxKind::WAIT_ORDER_STATEMENT => syntax::SyntaxKind::WAIT_ORDER_STATEMENT,
        slang::SyntaxKind::WAIT_STATEMENT => syntax::SyntaxKind::WAIT_STATEMENT,
        slang::SyntaxKind::WILDCARD_DIMENSION_SPECIFIER => {
            syntax::SyntaxKind::WILDCARD_DIMENSION_SPECIFIER
        }
        slang::SyntaxKind::WILDCARD_EQUALITY_EXPRESSION => {
            syntax::SyntaxKind::WILDCARD_EQUALITY_EXPRESSION
        }
        slang::SyntaxKind::WILDCARD_INEQUALITY_EXPRESSION => {
            syntax::SyntaxKind::WILDCARD_INEQUALITY_EXPRESSION
        }
        slang::SyntaxKind::WILDCARD_LITERAL_EXPRESSION => {
            syntax::SyntaxKind::WILDCARD_LITERAL_EXPRESSION
        }
        slang::SyntaxKind::WILDCARD_PATTERN => syntax::SyntaxKind::WILDCARD_PATTERN,
        slang::SyntaxKind::WILDCARD_PORT_CONNECTION => syntax::SyntaxKind::WILDCARD_PORT_CONNECTION,
        slang::SyntaxKind::WILDCARD_PORT_LIST => syntax::SyntaxKind::WILDCARD_PORT_LIST,
        slang::SyntaxKind::WILDCARD_UDP_PORT_LIST => syntax::SyntaxKind::WILDCARD_UDP_PORT_LIST,
        slang::SyntaxKind::WITH_CLAUSE => syntax::SyntaxKind::WITH_CLAUSE,
        slang::SyntaxKind::WITH_FUNCTION_CLAUSE => syntax::SyntaxKind::WITH_FUNCTION_CLAUSE,
        slang::SyntaxKind::WITH_FUNCTION_SAMPLE => syntax::SyntaxKind::WITH_FUNCTION_SAMPLE,
        slang::SyntaxKind::WITHIN_SEQUENCE_EXPR => syntax::SyntaxKind::WITHIN_SEQUENCE_EXPR,
        slang::SyntaxKind::XOR_ASSIGNMENT_EXPRESSION => {
            syntax::SyntaxKind::XOR_ASSIGNMENT_EXPRESSION
        }
        _ => syntax::SyntaxKind::UNKNOWN,
    }
}

pub(crate) fn owned_token_kind(kind: slang::TokenKind) -> syntax::TokenKind {
    match kind {
        slang::TokenKind::ACCEPT_ON_KEYWORD => syntax::TokenKind::ACCEPT_ON_KEYWORD,
        slang::TokenKind::ALIAS_KEYWORD => syntax::TokenKind::ALIAS_KEYWORD,
        slang::TokenKind::ALWAYS_COMB_KEYWORD => syntax::TokenKind::ALWAYS_COMB_KEYWORD,
        slang::TokenKind::ALWAYS_FF_KEYWORD => syntax::TokenKind::ALWAYS_FF_KEYWORD,
        slang::TokenKind::ALWAYS_KEYWORD => syntax::TokenKind::ALWAYS_KEYWORD,
        slang::TokenKind::ALWAYS_LATCH_KEYWORD => syntax::TokenKind::ALWAYS_LATCH_KEYWORD,
        slang::TokenKind::AND => syntax::TokenKind::AND,
        slang::TokenKind::AND_EQUAL => syntax::TokenKind::AND_EQUAL,
        slang::TokenKind::AND_KEYWORD => syntax::TokenKind::AND_KEYWORD,
        slang::TokenKind::APOSTROPHE => syntax::TokenKind::APOSTROPHE,
        slang::TokenKind::APOSTROPHE_OPEN_BRACE => syntax::TokenKind::APOSTROPHE_OPEN_BRACE,
        slang::TokenKind::ASSERT_KEYWORD => syntax::TokenKind::ASSERT_KEYWORD,
        slang::TokenKind::ASSIGN_KEYWORD => syntax::TokenKind::ASSIGN_KEYWORD,
        slang::TokenKind::ASSUME_KEYWORD => syntax::TokenKind::ASSUME_KEYWORD,
        slang::TokenKind::AT => syntax::TokenKind::AT,
        slang::TokenKind::AUTOMATIC_KEYWORD => syntax::TokenKind::AUTOMATIC_KEYWORD,
        slang::TokenKind::BEFORE_KEYWORD => syntax::TokenKind::BEFORE_KEYWORD,
        slang::TokenKind::BEGIN_KEYWORD => syntax::TokenKind::BEGIN_KEYWORD,
        slang::TokenKind::BIND_KEYWORD => syntax::TokenKind::BIND_KEYWORD,
        slang::TokenKind::BINS_KEYWORD => syntax::TokenKind::BINS_KEYWORD,
        slang::TokenKind::BINS_OF_KEYWORD => syntax::TokenKind::BINS_OF_KEYWORD,
        slang::TokenKind::BIT_KEYWORD => syntax::TokenKind::BIT_KEYWORD,
        slang::TokenKind::BREAK_KEYWORD => syntax::TokenKind::BREAK_KEYWORD,
        slang::TokenKind::BUF_IF_0_KEYWORD => syntax::TokenKind::BUF_IF_0_KEYWORD,
        slang::TokenKind::BUF_IF_1_KEYWORD => syntax::TokenKind::BUF_IF_1_KEYWORD,
        slang::TokenKind::BUF_KEYWORD => syntax::TokenKind::BUF_KEYWORD,
        slang::TokenKind::BYTE_KEYWORD => syntax::TokenKind::BYTE_KEYWORD,
        slang::TokenKind::C_HANDLE_KEYWORD => syntax::TokenKind::C_HANDLE_KEYWORD,
        slang::TokenKind::CASE_KEYWORD => syntax::TokenKind::CASE_KEYWORD,
        slang::TokenKind::CASE_X_KEYWORD => syntax::TokenKind::CASE_X_KEYWORD,
        slang::TokenKind::CASE_Z_KEYWORD => syntax::TokenKind::CASE_Z_KEYWORD,
        slang::TokenKind::CELL_KEYWORD => syntax::TokenKind::CELL_KEYWORD,
        slang::TokenKind::CHECKER_KEYWORD => syntax::TokenKind::CHECKER_KEYWORD,
        slang::TokenKind::CLASS_KEYWORD => syntax::TokenKind::CLASS_KEYWORD,
        slang::TokenKind::CLOCKING_KEYWORD => syntax::TokenKind::CLOCKING_KEYWORD,
        slang::TokenKind::CLOSE_BRACE => syntax::TokenKind::CLOSE_BRACE,
        slang::TokenKind::CLOSE_BRACKET => syntax::TokenKind::CLOSE_BRACKET,
        slang::TokenKind::CLOSE_PARENTHESIS => syntax::TokenKind::CLOSE_PARENTHESIS,
        slang::TokenKind::CMOS_KEYWORD => syntax::TokenKind::CMOS_KEYWORD,
        slang::TokenKind::COLON => syntax::TokenKind::COLON,
        slang::TokenKind::COLON_EQUALS => syntax::TokenKind::COLON_EQUALS,
        slang::TokenKind::COLON_SLASH => syntax::TokenKind::COLON_SLASH,
        slang::TokenKind::COMMA => syntax::TokenKind::COMMA,
        slang::TokenKind::CONFIG_KEYWORD => syntax::TokenKind::CONFIG_KEYWORD,
        slang::TokenKind::CONST_KEYWORD => syntax::TokenKind::CONST_KEYWORD,
        slang::TokenKind::CONSTRAINT_KEYWORD => syntax::TokenKind::CONSTRAINT_KEYWORD,
        slang::TokenKind::CONTEXT_KEYWORD => syntax::TokenKind::CONTEXT_KEYWORD,
        slang::TokenKind::CONTINUE_KEYWORD => syntax::TokenKind::CONTINUE_KEYWORD,
        slang::TokenKind::COVER_GROUP_KEYWORD => syntax::TokenKind::COVER_GROUP_KEYWORD,
        slang::TokenKind::COVER_KEYWORD => syntax::TokenKind::COVER_KEYWORD,
        slang::TokenKind::COVER_POINT_KEYWORD => syntax::TokenKind::COVER_POINT_KEYWORD,
        slang::TokenKind::CROSS_KEYWORD => syntax::TokenKind::CROSS_KEYWORD,
        slang::TokenKind::DEASSIGN_KEYWORD => syntax::TokenKind::DEASSIGN_KEYWORD,
        slang::TokenKind::DEF_PARAM_KEYWORD => syntax::TokenKind::DEF_PARAM_KEYWORD,
        slang::TokenKind::DEFAULT_KEYWORD => syntax::TokenKind::DEFAULT_KEYWORD,
        slang::TokenKind::DESIGN_KEYWORD => syntax::TokenKind::DESIGN_KEYWORD,
        slang::TokenKind::DIRECTIVE => syntax::TokenKind::DIRECTIVE,
        slang::TokenKind::DISABLE_KEYWORD => syntax::TokenKind::DISABLE_KEYWORD,
        slang::TokenKind::DIST_KEYWORD => syntax::TokenKind::DIST_KEYWORD,
        slang::TokenKind::DO_KEYWORD => syntax::TokenKind::DO_KEYWORD,
        slang::TokenKind::DOLLAR => syntax::TokenKind::DOLLAR,
        slang::TokenKind::DOT => syntax::TokenKind::DOT,
        slang::TokenKind::DOUBLE_AND => syntax::TokenKind::DOUBLE_AND,
        slang::TokenKind::DOUBLE_AT => syntax::TokenKind::DOUBLE_AT,
        slang::TokenKind::DOUBLE_COLON => syntax::TokenKind::DOUBLE_COLON,
        slang::TokenKind::DOUBLE_EQUALS => syntax::TokenKind::DOUBLE_EQUALS,
        slang::TokenKind::DOUBLE_EQUALS_QUESTION => syntax::TokenKind::DOUBLE_EQUALS_QUESTION,
        slang::TokenKind::DOUBLE_HASH => syntax::TokenKind::DOUBLE_HASH,
        slang::TokenKind::DOUBLE_MINUS => syntax::TokenKind::DOUBLE_MINUS,
        slang::TokenKind::DOUBLE_OR => syntax::TokenKind::DOUBLE_OR,
        slang::TokenKind::DOUBLE_PLUS => syntax::TokenKind::DOUBLE_PLUS,
        slang::TokenKind::DOUBLE_STAR => syntax::TokenKind::DOUBLE_STAR,
        slang::TokenKind::EDGE_KEYWORD => syntax::TokenKind::EDGE_KEYWORD,
        slang::TokenKind::ELSE_KEYWORD => syntax::TokenKind::ELSE_KEYWORD,
        slang::TokenKind::EMPTY_MACRO_ARGUMENT => syntax::TokenKind::EMPTY_MACRO_ARGUMENT,
        slang::TokenKind::END_CASE_KEYWORD => syntax::TokenKind::END_CASE_KEYWORD,
        slang::TokenKind::END_CHECKER_KEYWORD => syntax::TokenKind::END_CHECKER_KEYWORD,
        slang::TokenKind::END_CLASS_KEYWORD => syntax::TokenKind::END_CLASS_KEYWORD,
        slang::TokenKind::END_CLOCKING_KEYWORD => syntax::TokenKind::END_CLOCKING_KEYWORD,
        slang::TokenKind::END_CONFIG_KEYWORD => syntax::TokenKind::END_CONFIG_KEYWORD,
        slang::TokenKind::END_FUNCTION_KEYWORD => syntax::TokenKind::END_FUNCTION_KEYWORD,
        slang::TokenKind::END_GENERATE_KEYWORD => syntax::TokenKind::END_GENERATE_KEYWORD,
        slang::TokenKind::END_GROUP_KEYWORD => syntax::TokenKind::END_GROUP_KEYWORD,
        slang::TokenKind::END_INTERFACE_KEYWORD => syntax::TokenKind::END_INTERFACE_KEYWORD,
        slang::TokenKind::END_KEYWORD => syntax::TokenKind::END_KEYWORD,
        slang::TokenKind::END_MODULE_KEYWORD => syntax::TokenKind::END_MODULE_KEYWORD,
        slang::TokenKind::END_OF_FILE => syntax::TokenKind::END_OF_FILE,
        slang::TokenKind::END_PACKAGE_KEYWORD => syntax::TokenKind::END_PACKAGE_KEYWORD,
        slang::TokenKind::END_PRIMITIVE_KEYWORD => syntax::TokenKind::END_PRIMITIVE_KEYWORD,
        slang::TokenKind::END_PROGRAM_KEYWORD => syntax::TokenKind::END_PROGRAM_KEYWORD,
        slang::TokenKind::END_PROPERTY_KEYWORD => syntax::TokenKind::END_PROPERTY_KEYWORD,
        slang::TokenKind::END_SEQUENCE_KEYWORD => syntax::TokenKind::END_SEQUENCE_KEYWORD,
        slang::TokenKind::END_SPECIFY_KEYWORD => syntax::TokenKind::END_SPECIFY_KEYWORD,
        slang::TokenKind::END_TABLE_KEYWORD => syntax::TokenKind::END_TABLE_KEYWORD,
        slang::TokenKind::END_TASK_KEYWORD => syntax::TokenKind::END_TASK_KEYWORD,
        slang::TokenKind::ENUM_KEYWORD => syntax::TokenKind::ENUM_KEYWORD,
        slang::TokenKind::EQUALS => syntax::TokenKind::EQUALS,
        slang::TokenKind::EQUALS_ARROW => syntax::TokenKind::EQUALS_ARROW,
        slang::TokenKind::EVENT_KEYWORD => syntax::TokenKind::EVENT_KEYWORD,
        slang::TokenKind::EVENTUALLY_KEYWORD => syntax::TokenKind::EVENTUALLY_KEYWORD,
        slang::TokenKind::EXCLAMATION => syntax::TokenKind::EXCLAMATION,
        slang::TokenKind::EXCLAMATION_DOUBLE_EQUALS => syntax::TokenKind::EXCLAMATION_DOUBLE_EQUALS,
        slang::TokenKind::EXCLAMATION_EQUALS => syntax::TokenKind::EXCLAMATION_EQUALS,
        slang::TokenKind::EXCLAMATION_EQUALS_QUESTION => {
            syntax::TokenKind::EXCLAMATION_EQUALS_QUESTION
        }
        slang::TokenKind::EXPECT_KEYWORD => syntax::TokenKind::EXPECT_KEYWORD,
        slang::TokenKind::EXPORT_KEYWORD => syntax::TokenKind::EXPORT_KEYWORD,
        slang::TokenKind::EXTENDS_KEYWORD => syntax::TokenKind::EXTENDS_KEYWORD,
        slang::TokenKind::EXTERN_KEYWORD => syntax::TokenKind::EXTERN_KEYWORD,
        slang::TokenKind::FINAL_KEYWORD => syntax::TokenKind::FINAL_KEYWORD,
        slang::TokenKind::FIRST_MATCH_KEYWORD => syntax::TokenKind::FIRST_MATCH_KEYWORD,
        slang::TokenKind::FOR_KEYWORD => syntax::TokenKind::FOR_KEYWORD,
        slang::TokenKind::FORCE_KEYWORD => syntax::TokenKind::FORCE_KEYWORD,
        slang::TokenKind::FOREACH_KEYWORD => syntax::TokenKind::FOREACH_KEYWORD,
        slang::TokenKind::FOREVER_KEYWORD => syntax::TokenKind::FOREVER_KEYWORD,
        slang::TokenKind::FORK_JOIN_KEYWORD => syntax::TokenKind::FORK_JOIN_KEYWORD,
        slang::TokenKind::FORK_KEYWORD => syntax::TokenKind::FORK_KEYWORD,
        slang::TokenKind::FUNCTION_KEYWORD => syntax::TokenKind::FUNCTION_KEYWORD,
        slang::TokenKind::GEN_VAR_KEYWORD => syntax::TokenKind::GEN_VAR_KEYWORD,
        slang::TokenKind::GENERATE_KEYWORD => syntax::TokenKind::GENERATE_KEYWORD,
        slang::TokenKind::GLOBAL_KEYWORD => syntax::TokenKind::GLOBAL_KEYWORD,
        slang::TokenKind::GREATER_THAN => syntax::TokenKind::GREATER_THAN,
        slang::TokenKind::GREATER_THAN_EQUALS => syntax::TokenKind::GREATER_THAN_EQUALS,
        slang::TokenKind::HASH => syntax::TokenKind::HASH,
        slang::TokenKind::HASH_EQUALS_HASH => syntax::TokenKind::HASH_EQUALS_HASH,
        slang::TokenKind::HASH_MINUS_HASH => syntax::TokenKind::HASH_MINUS_HASH,
        slang::TokenKind::HIGH_Z0_KEYWORD => syntax::TokenKind::HIGH_Z0_KEYWORD,
        slang::TokenKind::HIGH_Z1_KEYWORD => syntax::TokenKind::HIGH_Z1_KEYWORD,
        slang::TokenKind::IDENTIFIER => syntax::TokenKind::IDENTIFIER,
        slang::TokenKind::IF_KEYWORD => syntax::TokenKind::IF_KEYWORD,
        slang::TokenKind::IF_NONE_KEYWORD => syntax::TokenKind::IF_NONE_KEYWORD,
        slang::TokenKind::IFF_KEYWORD => syntax::TokenKind::IFF_KEYWORD,
        slang::TokenKind::IGNORE_BINS_KEYWORD => syntax::TokenKind::IGNORE_BINS_KEYWORD,
        slang::TokenKind::ILLEGAL_BINS_KEYWORD => syntax::TokenKind::ILLEGAL_BINS_KEYWORD,
        slang::TokenKind::IMPLEMENTS_KEYWORD => syntax::TokenKind::IMPLEMENTS_KEYWORD,
        slang::TokenKind::IMPLIES_KEYWORD => syntax::TokenKind::IMPLIES_KEYWORD,
        slang::TokenKind::IMPORT_KEYWORD => syntax::TokenKind::IMPORT_KEYWORD,
        slang::TokenKind::IN_OUT_KEYWORD => syntax::TokenKind::IN_OUT_KEYWORD,
        slang::TokenKind::INC_DIR_KEYWORD => syntax::TokenKind::INC_DIR_KEYWORD,
        slang::TokenKind::INCLUDE_FILE_NAME => syntax::TokenKind::INCLUDE_FILE_NAME,
        slang::TokenKind::INCLUDE_KEYWORD => syntax::TokenKind::INCLUDE_KEYWORD,
        slang::TokenKind::INITIAL_KEYWORD => syntax::TokenKind::INITIAL_KEYWORD,
        slang::TokenKind::INPUT_KEYWORD => syntax::TokenKind::INPUT_KEYWORD,
        slang::TokenKind::INSIDE_KEYWORD => syntax::TokenKind::INSIDE_KEYWORD,
        slang::TokenKind::INSTANCE_KEYWORD => syntax::TokenKind::INSTANCE_KEYWORD,
        slang::TokenKind::INT_KEYWORD => syntax::TokenKind::INT_KEYWORD,
        slang::TokenKind::INTEGER_BASE => syntax::TokenKind::INTEGER_BASE,
        slang::TokenKind::INTEGER_KEYWORD => syntax::TokenKind::INTEGER_KEYWORD,
        slang::TokenKind::INTEGER_LITERAL => syntax::TokenKind::INTEGER_LITERAL,
        slang::TokenKind::INTERCONNECT_KEYWORD => syntax::TokenKind::INTERCONNECT_KEYWORD,
        slang::TokenKind::INTERFACE_KEYWORD => syntax::TokenKind::INTERFACE_KEYWORD,
        slang::TokenKind::INTERSECT_KEYWORD => syntax::TokenKind::INTERSECT_KEYWORD,
        slang::TokenKind::JOIN_ANY_KEYWORD => syntax::TokenKind::JOIN_ANY_KEYWORD,
        slang::TokenKind::JOIN_KEYWORD => syntax::TokenKind::JOIN_KEYWORD,
        slang::TokenKind::JOIN_NONE_KEYWORD => syntax::TokenKind::JOIN_NONE_KEYWORD,
        slang::TokenKind::LARGE_KEYWORD => syntax::TokenKind::LARGE_KEYWORD,
        slang::TokenKind::LEFT_SHIFT => syntax::TokenKind::LEFT_SHIFT,
        slang::TokenKind::LEFT_SHIFT_EQUAL => syntax::TokenKind::LEFT_SHIFT_EQUAL,
        slang::TokenKind::LESS_THAN => syntax::TokenKind::LESS_THAN,
        slang::TokenKind::LESS_THAN_EQUALS => syntax::TokenKind::LESS_THAN_EQUALS,
        slang::TokenKind::LESS_THAN_MINUS_ARROW => syntax::TokenKind::LESS_THAN_MINUS_ARROW,
        slang::TokenKind::LET_KEYWORD => syntax::TokenKind::LET_KEYWORD,
        slang::TokenKind::LIB_LIST_KEYWORD => syntax::TokenKind::LIB_LIST_KEYWORD,
        slang::TokenKind::LIBRARY_KEYWORD => syntax::TokenKind::LIBRARY_KEYWORD,
        slang::TokenKind::LINE_CONTINUATION => syntax::TokenKind::LINE_CONTINUATION,
        slang::TokenKind::LOCAL_KEYWORD => syntax::TokenKind::LOCAL_KEYWORD,
        slang::TokenKind::LOCAL_PARAM_KEYWORD => syntax::TokenKind::LOCAL_PARAM_KEYWORD,
        slang::TokenKind::LOGIC_KEYWORD => syntax::TokenKind::LOGIC_KEYWORD,
        slang::TokenKind::LONG_INT_KEYWORD => syntax::TokenKind::LONG_INT_KEYWORD,
        slang::TokenKind::MACRO_ESCAPED_QUOTE => syntax::TokenKind::MACRO_ESCAPED_QUOTE,
        slang::TokenKind::MACRO_PASTE => syntax::TokenKind::MACRO_PASTE,
        slang::TokenKind::MACRO_QUOTE => syntax::TokenKind::MACRO_QUOTE,
        slang::TokenKind::MACRO_TRIPLE_QUOTE => syntax::TokenKind::MACRO_TRIPLE_QUOTE,
        slang::TokenKind::MACRO_USAGE => syntax::TokenKind::MACRO_USAGE,
        slang::TokenKind::MACROMODULE_KEYWORD => syntax::TokenKind::MACROMODULE_KEYWORD,
        slang::TokenKind::MATCHES_KEYWORD => syntax::TokenKind::MATCHES_KEYWORD,
        slang::TokenKind::MEDIUM_KEYWORD => syntax::TokenKind::MEDIUM_KEYWORD,
        slang::TokenKind::MINUS => syntax::TokenKind::MINUS,
        slang::TokenKind::MINUS_ARROW => syntax::TokenKind::MINUS_ARROW,
        slang::TokenKind::MINUS_COLON => syntax::TokenKind::MINUS_COLON,
        slang::TokenKind::MINUS_DOUBLE_ARROW => syntax::TokenKind::MINUS_DOUBLE_ARROW,
        slang::TokenKind::MINUS_EQUAL => syntax::TokenKind::MINUS_EQUAL,
        slang::TokenKind::MOD_PORT_KEYWORD => syntax::TokenKind::MOD_PORT_KEYWORD,
        slang::TokenKind::MODULE_KEYWORD => syntax::TokenKind::MODULE_KEYWORD,
        slang::TokenKind::NAND_KEYWORD => syntax::TokenKind::NAND_KEYWORD,
        slang::TokenKind::NEG_EDGE_KEYWORD => syntax::TokenKind::NEG_EDGE_KEYWORD,
        slang::TokenKind::NET_TYPE_KEYWORD => syntax::TokenKind::NET_TYPE_KEYWORD,
        slang::TokenKind::NEW_KEYWORD => syntax::TokenKind::NEW_KEYWORD,
        slang::TokenKind::NEXT_TIME_KEYWORD => syntax::TokenKind::NEXT_TIME_KEYWORD,
        slang::TokenKind::NMOS_KEYWORD => syntax::TokenKind::NMOS_KEYWORD,
        slang::TokenKind::NO_SHOW_CANCELLED_KEYWORD => syntax::TokenKind::NO_SHOW_CANCELLED_KEYWORD,
        slang::TokenKind::NOR_KEYWORD => syntax::TokenKind::NOR_KEYWORD,
        slang::TokenKind::NOT_IF_0_KEYWORD => syntax::TokenKind::NOT_IF_0_KEYWORD,
        slang::TokenKind::NOT_IF_1_KEYWORD => syntax::TokenKind::NOT_IF_1_KEYWORD,
        slang::TokenKind::NOT_KEYWORD => syntax::TokenKind::NOT_KEYWORD,
        slang::TokenKind::NULL_KEYWORD => syntax::TokenKind::NULL_KEYWORD,
        slang::TokenKind::ONE_STEP => syntax::TokenKind::ONE_STEP,
        slang::TokenKind::OPEN_BRACE => syntax::TokenKind::OPEN_BRACE,
        slang::TokenKind::OPEN_BRACKET => syntax::TokenKind::OPEN_BRACKET,
        slang::TokenKind::OPEN_PARENTHESIS => syntax::TokenKind::OPEN_PARENTHESIS,
        slang::TokenKind::OR => syntax::TokenKind::OR,
        slang::TokenKind::OR_EQUAL => syntax::TokenKind::OR_EQUAL,
        slang::TokenKind::OR_EQUALS_ARROW => syntax::TokenKind::OR_EQUALS_ARROW,
        slang::TokenKind::OR_KEYWORD => syntax::TokenKind::OR_KEYWORD,
        slang::TokenKind::OR_MINUS_ARROW => syntax::TokenKind::OR_MINUS_ARROW,
        slang::TokenKind::OUTPUT_KEYWORD => syntax::TokenKind::OUTPUT_KEYWORD,
        slang::TokenKind::PACKAGE_KEYWORD => syntax::TokenKind::PACKAGE_KEYWORD,
        slang::TokenKind::PACKED_KEYWORD => syntax::TokenKind::PACKED_KEYWORD,
        slang::TokenKind::PARAMETER_KEYWORD => syntax::TokenKind::PARAMETER_KEYWORD,
        slang::TokenKind::PERCENT => syntax::TokenKind::PERCENT,
        slang::TokenKind::PERCENT_EQUAL => syntax::TokenKind::PERCENT_EQUAL,
        slang::TokenKind::PLACEHOLDER => syntax::TokenKind::PLACEHOLDER,
        slang::TokenKind::PLUS => syntax::TokenKind::PLUS,
        slang::TokenKind::PLUS_COLON => syntax::TokenKind::PLUS_COLON,
        slang::TokenKind::PLUS_DIV_MINUS => syntax::TokenKind::PLUS_DIV_MINUS,
        slang::TokenKind::PLUS_EQUAL => syntax::TokenKind::PLUS_EQUAL,
        slang::TokenKind::PLUS_MOD_MINUS => syntax::TokenKind::PLUS_MOD_MINUS,
        slang::TokenKind::PMOS_KEYWORD => syntax::TokenKind::PMOS_KEYWORD,
        slang::TokenKind::POS_EDGE_KEYWORD => syntax::TokenKind::POS_EDGE_KEYWORD,
        slang::TokenKind::PRIMITIVE_KEYWORD => syntax::TokenKind::PRIMITIVE_KEYWORD,
        slang::TokenKind::PRIORITY_KEYWORD => syntax::TokenKind::PRIORITY_KEYWORD,
        slang::TokenKind::PROGRAM_KEYWORD => syntax::TokenKind::PROGRAM_KEYWORD,
        slang::TokenKind::PROPERTY_KEYWORD => syntax::TokenKind::PROPERTY_KEYWORD,
        slang::TokenKind::PROTECTED_KEYWORD => syntax::TokenKind::PROTECTED_KEYWORD,
        slang::TokenKind::PULL_0_KEYWORD => syntax::TokenKind::PULL_0_KEYWORD,
        slang::TokenKind::PULL_1_KEYWORD => syntax::TokenKind::PULL_1_KEYWORD,
        slang::TokenKind::PULL_DOWN_KEYWORD => syntax::TokenKind::PULL_DOWN_KEYWORD,
        slang::TokenKind::PULL_UP_KEYWORD => syntax::TokenKind::PULL_UP_KEYWORD,
        slang::TokenKind::PULSE_STYLE_ON_DETECT_KEYWORD => {
            syntax::TokenKind::PULSE_STYLE_ON_DETECT_KEYWORD
        }
        slang::TokenKind::PULSE_STYLE_ON_EVENT_KEYWORD => {
            syntax::TokenKind::PULSE_STYLE_ON_EVENT_KEYWORD
        }
        slang::TokenKind::PURE_KEYWORD => syntax::TokenKind::PURE_KEYWORD,
        slang::TokenKind::QUESTION => syntax::TokenKind::QUESTION,
        slang::TokenKind::RAND_C_KEYWORD => syntax::TokenKind::RAND_C_KEYWORD,
        slang::TokenKind::RAND_CASE_KEYWORD => syntax::TokenKind::RAND_CASE_KEYWORD,
        slang::TokenKind::RAND_KEYWORD => syntax::TokenKind::RAND_KEYWORD,
        slang::TokenKind::RAND_SEQUENCE_KEYWORD => syntax::TokenKind::RAND_SEQUENCE_KEYWORD,
        slang::TokenKind::RCMOS_KEYWORD => syntax::TokenKind::RCMOS_KEYWORD,
        slang::TokenKind::REAL_KEYWORD => syntax::TokenKind::REAL_KEYWORD,
        slang::TokenKind::REAL_LITERAL => syntax::TokenKind::REAL_LITERAL,
        slang::TokenKind::REAL_TIME_KEYWORD => syntax::TokenKind::REAL_TIME_KEYWORD,
        slang::TokenKind::REF_KEYWORD => syntax::TokenKind::REF_KEYWORD,
        slang::TokenKind::REG_KEYWORD => syntax::TokenKind::REG_KEYWORD,
        slang::TokenKind::REJECT_ON_KEYWORD => syntax::TokenKind::REJECT_ON_KEYWORD,
        slang::TokenKind::RELEASE_KEYWORD => syntax::TokenKind::RELEASE_KEYWORD,
        slang::TokenKind::REPEAT_KEYWORD => syntax::TokenKind::REPEAT_KEYWORD,
        slang::TokenKind::RESTRICT_KEYWORD => syntax::TokenKind::RESTRICT_KEYWORD,
        slang::TokenKind::RETURN_KEYWORD => syntax::TokenKind::RETURN_KEYWORD,
        slang::TokenKind::RIGHT_SHIFT => syntax::TokenKind::RIGHT_SHIFT,
        slang::TokenKind::RIGHT_SHIFT_EQUAL => syntax::TokenKind::RIGHT_SHIFT_EQUAL,
        slang::TokenKind::RNMOS_KEYWORD => syntax::TokenKind::RNMOS_KEYWORD,
        slang::TokenKind::ROOT_SYSTEM_NAME => syntax::TokenKind::ROOT_SYSTEM_NAME,
        slang::TokenKind::RPMOS_KEYWORD => syntax::TokenKind::RPMOS_KEYWORD,
        slang::TokenKind::RTRAN_IF_0_KEYWORD => syntax::TokenKind::RTRAN_IF_0_KEYWORD,
        slang::TokenKind::RTRAN_IF_1_KEYWORD => syntax::TokenKind::RTRAN_IF_1_KEYWORD,
        slang::TokenKind::RTRAN_KEYWORD => syntax::TokenKind::RTRAN_KEYWORD,
        slang::TokenKind::S_ALWAYS_KEYWORD => syntax::TokenKind::S_ALWAYS_KEYWORD,
        slang::TokenKind::S_EVENTUALLY_KEYWORD => syntax::TokenKind::S_EVENTUALLY_KEYWORD,
        slang::TokenKind::S_NEXT_TIME_KEYWORD => syntax::TokenKind::S_NEXT_TIME_KEYWORD,
        slang::TokenKind::S_UNTIL_KEYWORD => syntax::TokenKind::S_UNTIL_KEYWORD,
        slang::TokenKind::S_UNTIL_WITH_KEYWORD => syntax::TokenKind::S_UNTIL_WITH_KEYWORD,
        slang::TokenKind::SCALARED_KEYWORD => syntax::TokenKind::SCALARED_KEYWORD,
        slang::TokenKind::SEMICOLON => syntax::TokenKind::SEMICOLON,
        slang::TokenKind::SEQUENCE_KEYWORD => syntax::TokenKind::SEQUENCE_KEYWORD,
        slang::TokenKind::SHORT_INT_KEYWORD => syntax::TokenKind::SHORT_INT_KEYWORD,
        slang::TokenKind::SHORT_REAL_KEYWORD => syntax::TokenKind::SHORT_REAL_KEYWORD,
        slang::TokenKind::SHOW_CANCELLED_KEYWORD => syntax::TokenKind::SHOW_CANCELLED_KEYWORD,
        slang::TokenKind::SIGNED_KEYWORD => syntax::TokenKind::SIGNED_KEYWORD,
        slang::TokenKind::SLASH => syntax::TokenKind::SLASH,
        slang::TokenKind::SLASH_EQUAL => syntax::TokenKind::SLASH_EQUAL,
        slang::TokenKind::SMALL_KEYWORD => syntax::TokenKind::SMALL_KEYWORD,
        slang::TokenKind::SOFT_KEYWORD => syntax::TokenKind::SOFT_KEYWORD,
        slang::TokenKind::SOLVE_KEYWORD => syntax::TokenKind::SOLVE_KEYWORD,
        slang::TokenKind::SPEC_PARAM_KEYWORD => syntax::TokenKind::SPEC_PARAM_KEYWORD,
        slang::TokenKind::SPECIFY_KEYWORD => syntax::TokenKind::SPECIFY_KEYWORD,
        slang::TokenKind::STAR => syntax::TokenKind::STAR,
        slang::TokenKind::STAR_ARROW => syntax::TokenKind::STAR_ARROW,
        slang::TokenKind::STAR_EQUAL => syntax::TokenKind::STAR_EQUAL,
        slang::TokenKind::STATIC_KEYWORD => syntax::TokenKind::STATIC_KEYWORD,
        slang::TokenKind::STRING_KEYWORD => syntax::TokenKind::STRING_KEYWORD,
        slang::TokenKind::STRING_LITERAL => syntax::TokenKind::STRING_LITERAL,
        slang::TokenKind::STRONG_0_KEYWORD => syntax::TokenKind::STRONG_0_KEYWORD,
        slang::TokenKind::STRONG_1_KEYWORD => syntax::TokenKind::STRONG_1_KEYWORD,
        slang::TokenKind::STRONG_KEYWORD => syntax::TokenKind::STRONG_KEYWORD,
        slang::TokenKind::STRUCT_KEYWORD => syntax::TokenKind::STRUCT_KEYWORD,
        slang::TokenKind::SUPER_KEYWORD => syntax::TokenKind::SUPER_KEYWORD,
        slang::TokenKind::SUPPLY_0_KEYWORD => syntax::TokenKind::SUPPLY_0_KEYWORD,
        slang::TokenKind::SUPPLY_1_KEYWORD => syntax::TokenKind::SUPPLY_1_KEYWORD,
        slang::TokenKind::SYNC_ACCEPT_ON_KEYWORD => syntax::TokenKind::SYNC_ACCEPT_ON_KEYWORD,
        slang::TokenKind::SYNC_REJECT_ON_KEYWORD => syntax::TokenKind::SYNC_REJECT_ON_KEYWORD,
        slang::TokenKind::SYSTEM_IDENTIFIER => syntax::TokenKind::SYSTEM_IDENTIFIER,
        slang::TokenKind::TABLE_KEYWORD => syntax::TokenKind::TABLE_KEYWORD,
        slang::TokenKind::TAGGED_KEYWORD => syntax::TokenKind::TAGGED_KEYWORD,
        slang::TokenKind::TASK_KEYWORD => syntax::TokenKind::TASK_KEYWORD,
        slang::TokenKind::THIS_KEYWORD => syntax::TokenKind::THIS_KEYWORD,
        slang::TokenKind::THROUGHOUT_KEYWORD => syntax::TokenKind::THROUGHOUT_KEYWORD,
        slang::TokenKind::TILDE => syntax::TokenKind::TILDE,
        slang::TokenKind::TILDE_AND => syntax::TokenKind::TILDE_AND,
        slang::TokenKind::TILDE_OR => syntax::TokenKind::TILDE_OR,
        slang::TokenKind::TILDE_XOR => syntax::TokenKind::TILDE_XOR,
        slang::TokenKind::TIME_KEYWORD => syntax::TokenKind::TIME_KEYWORD,
        slang::TokenKind::TIME_LITERAL => syntax::TokenKind::TIME_LITERAL,
        slang::TokenKind::TIME_PRECISION_KEYWORD => syntax::TokenKind::TIME_PRECISION_KEYWORD,
        slang::TokenKind::TIME_UNIT_KEYWORD => syntax::TokenKind::TIME_UNIT_KEYWORD,
        slang::TokenKind::TRAN_IF_0_KEYWORD => syntax::TokenKind::TRAN_IF_0_KEYWORD,
        slang::TokenKind::TRAN_IF_1_KEYWORD => syntax::TokenKind::TRAN_IF_1_KEYWORD,
        slang::TokenKind::TRAN_KEYWORD => syntax::TokenKind::TRAN_KEYWORD,
        slang::TokenKind::TRI_0_KEYWORD => syntax::TokenKind::TRI_0_KEYWORD,
        slang::TokenKind::TRI_1_KEYWORD => syntax::TokenKind::TRI_1_KEYWORD,
        slang::TokenKind::TRI_AND_KEYWORD => syntax::TokenKind::TRI_AND_KEYWORD,
        slang::TokenKind::TRI_KEYWORD => syntax::TokenKind::TRI_KEYWORD,
        slang::TokenKind::TRI_OR_KEYWORD => syntax::TokenKind::TRI_OR_KEYWORD,
        slang::TokenKind::TRI_REG_KEYWORD => syntax::TokenKind::TRI_REG_KEYWORD,
        slang::TokenKind::TRIPLE_AND => syntax::TokenKind::TRIPLE_AND,
        slang::TokenKind::TRIPLE_EQUALS => syntax::TokenKind::TRIPLE_EQUALS,
        slang::TokenKind::TRIPLE_LEFT_SHIFT => syntax::TokenKind::TRIPLE_LEFT_SHIFT,
        slang::TokenKind::TRIPLE_LEFT_SHIFT_EQUAL => syntax::TokenKind::TRIPLE_LEFT_SHIFT_EQUAL,
        slang::TokenKind::TRIPLE_RIGHT_SHIFT => syntax::TokenKind::TRIPLE_RIGHT_SHIFT,
        slang::TokenKind::TRIPLE_RIGHT_SHIFT_EQUAL => syntax::TokenKind::TRIPLE_RIGHT_SHIFT_EQUAL,
        slang::TokenKind::TYPE_KEYWORD => syntax::TokenKind::TYPE_KEYWORD,
        slang::TokenKind::TYPEDEF_KEYWORD => syntax::TokenKind::TYPEDEF_KEYWORD,
        slang::TokenKind::U_WIRE_KEYWORD => syntax::TokenKind::U_WIRE_KEYWORD,
        slang::TokenKind::UNBASED_UNSIZED_LITERAL => syntax::TokenKind::UNBASED_UNSIZED_LITERAL,
        slang::TokenKind::UNION_KEYWORD => syntax::TokenKind::UNION_KEYWORD,
        slang::TokenKind::UNIQUE_0_KEYWORD => syntax::TokenKind::UNIQUE_0_KEYWORD,
        slang::TokenKind::UNIQUE_KEYWORD => syntax::TokenKind::UNIQUE_KEYWORD,
        slang::TokenKind::UNIT_SYSTEM_NAME => syntax::TokenKind::UNIT_SYSTEM_NAME,
        slang::TokenKind::UNKNOWN => syntax::TokenKind::UNKNOWN,
        slang::TokenKind::UNSIGNED_KEYWORD => syntax::TokenKind::UNSIGNED_KEYWORD,
        slang::TokenKind::UNTIL_KEYWORD => syntax::TokenKind::UNTIL_KEYWORD,
        slang::TokenKind::UNTIL_WITH_KEYWORD => syntax::TokenKind::UNTIL_WITH_KEYWORD,
        slang::TokenKind::UNTYPED_KEYWORD => syntax::TokenKind::UNTYPED_KEYWORD,
        slang::TokenKind::USE_KEYWORD => syntax::TokenKind::USE_KEYWORD,
        slang::TokenKind::VAR_KEYWORD => syntax::TokenKind::VAR_KEYWORD,
        slang::TokenKind::VECTORED_KEYWORD => syntax::TokenKind::VECTORED_KEYWORD,
        slang::TokenKind::VIRTUAL_KEYWORD => syntax::TokenKind::VIRTUAL_KEYWORD,
        slang::TokenKind::VOID_KEYWORD => syntax::TokenKind::VOID_KEYWORD,
        slang::TokenKind::W_AND_KEYWORD => syntax::TokenKind::W_AND_KEYWORD,
        slang::TokenKind::W_OR_KEYWORD => syntax::TokenKind::W_OR_KEYWORD,
        slang::TokenKind::WAIT_KEYWORD => syntax::TokenKind::WAIT_KEYWORD,
        slang::TokenKind::WAIT_ORDER_KEYWORD => syntax::TokenKind::WAIT_ORDER_KEYWORD,
        slang::TokenKind::WEAK_0_KEYWORD => syntax::TokenKind::WEAK_0_KEYWORD,
        slang::TokenKind::WEAK_1_KEYWORD => syntax::TokenKind::WEAK_1_KEYWORD,
        slang::TokenKind::WEAK_KEYWORD => syntax::TokenKind::WEAK_KEYWORD,
        slang::TokenKind::WHILE_KEYWORD => syntax::TokenKind::WHILE_KEYWORD,
        slang::TokenKind::WILDCARD_KEYWORD => syntax::TokenKind::WILDCARD_KEYWORD,
        slang::TokenKind::WIRE_KEYWORD => syntax::TokenKind::WIRE_KEYWORD,
        slang::TokenKind::WITH_KEYWORD => syntax::TokenKind::WITH_KEYWORD,
        slang::TokenKind::WITHIN_KEYWORD => syntax::TokenKind::WITHIN_KEYWORD,
        slang::TokenKind::XNOR_KEYWORD => syntax::TokenKind::XNOR_KEYWORD,
        slang::TokenKind::XOR => syntax::TokenKind::XOR,
        slang::TokenKind::XOR_EQUAL => syntax::TokenKind::XOR_EQUAL,
        slang::TokenKind::XOR_KEYWORD => syntax::TokenKind::XOR_KEYWORD,
        slang::TokenKind::XOR_TILDE => syntax::TokenKind::XOR_TILDE,
        _ => syntax::TokenKind::UNKNOWN,
    }
}
