#[macro_export]
macro_rules! match_ast {
    ($node:expr , _ => $body:expr,) => { $body };

    ($node:expr , $path:ty[$it:pat] $(if $cond:expr)? => $body:expr, $($rest:tt)* ) => {{
        if let Some($it) = <$path as $crate::ast::AstNode>::cast($node)
        $( && ($cond) )? {
            $body
        } else {
            match_ast!($node , $($rest)*)
        }
    }};

    ($node:expr , $path:ty $(| $paths:ty)* => $body:expr, $($rest:tt)* ) => {{
        if <$path as $crate::ast::AstNode>::cast($node).is_some() $(|| <$paths as $crate::ast::AstNode>::cast($node).is_some())* {
            $body
        } else {
            match_ast!($node , $($rest)*)
        }
    }}
}

#[macro_export]
macro_rules! match_ast_kind {
    ($kind:expr , _ => $body:expr,) => { $body };

    ($kind:expr , $path:ty $(where $cond:expr)? => $body:expr, $($rest:tt)* ) => {{
        if <$path as $crate::ast::AstNode>::can_cast($kind)
        $( && ($cond) )? {
            $body
        } else {
            match_ast_kind!($kind , $($rest)*)
        }
    }};

    ($kind:expr , $path:ty $(| $paths:ty)* => $body:expr, $($rest:tt)* ) => {{
        if <$path as $crate::ast::AstNode>::can_cast($kind) $(|| <$paths as $crate::ast::AstNode>::can_cast($kind))* {
            $body
        } else {
            match_ast_kind!($kind , $($rest)*)
        }
    }}
}

#[macro_export]
macro_rules! Token {
    [.] => { $crate::TokenKind::DOT };
    [::] => { $crate::TokenKind::DOUBLE_COLON };
    [#] => { $crate::TokenKind::HASH };
    [@] => { $crate::TokenKind::AT };
    [","] => { $crate::TokenKind::COMMA };
    ["'"] => { $crate::TokenKind::APOSTROPHE };
    [+] => { $crate::TokenKind::PLUS };
    [+=] => { $crate::TokenKind::PLUS_EQUAL };
    [->] => { $crate::TokenKind::MINUS_ARROW };
    [&&] => { $crate::TokenKind::DOUBLE_AND };
    [**] => { $crate::TokenKind::DOUBLE_STAR };
    [:=] => { $crate::TokenKind::COLON_EQUALS };
    [>=] => { $crate::TokenKind::GREATER_THAN_EQUALS };
    ["$unit"] => { $crate::TokenKind::UNIT_SYSTEM_NAME };
    ["$root"] => { $crate::TokenKind::ROOT_SYSTEM_NAME };
    ["`\""] => { $crate::TokenKind::MACRO_QUOTE };
    ["``"] => { $crate::TokenKind::MACRO_PASTE };
    [always] => { $crate::TokenKind::ALWAYS_KEYWORD };
    [and] => { $crate::TokenKind::AND_KEYWORD };
    [assign] => { $crate::TokenKind::ASSIGN_KEYWORD };
    [begin] => { $crate::TokenKind::BEGIN_KEYWORD };
    [bufif0] => { $crate::TokenKind::BUFIF_0_KEYWORD };
    [case] => { $crate::TokenKind::CASE_KEYWORD };
    [casex] => { $crate::TokenKind::CASE_X_KEYWORD };
    [default] => { $crate::TokenKind::DEFAULT_KEYWORD };
    [design] => { $crate::TokenKind::DESIGN_KEYWORD };
    [edge] => { $crate::TokenKind::EDGE_KEYWORD };
    [event] => { $crate::TokenKind::EVENT_KEYWORD };
    [if] => { $crate::TokenKind::IF_KEYWORD };
    [input] => { $crate::TokenKind::INPUT_KEYWORD };
    [integer] => { $crate::TokenKind::INTEGER_KEYWORD };
    [library] => { $crate::TokenKind::LIBRARY_KEYWORD };
    [localparam] => { $crate::TokenKind::LOCAL_PARAM_KEYWORD };
    [module] => { $crate::TokenKind::MODULE_KEYWORD };
    [output] => { $crate::TokenKind::OUTPUT_KEYWORD };
    [parameter] => { $crate::TokenKind::PARAMETER_KEYWORD };
    [posedge] => { $crate::TokenKind::POSEDGE_KEYWORD };
    [pulsestyle_ondetect] => { $crate::TokenKind::PULSE_STYLE_ON_DETECT_KEYWORD };
    [wire] => { $crate::TokenKind::WIRE_KEYWORD };
    [function] => { $crate::TokenKind::FUNCTION_KEYWORD };
    [endmodule] => { $crate::TokenKind::END_MODULE_KEYWORD };
}

#[macro_export]
macro_rules! Trivia {
    [ws] => { $crate::TriviaKind::WHITESPACE };
    [eol] => { $crate::TriviaKind::END_OF_LINE };
    [lc] => { $crate::TriviaKind::LINE_COMMENT };
    [bc] => { $crate::TriviaKind::BLOCK_COMMENT };
    ["`"] => { $crate::TriviaKind::DIRECTIVE };
}
