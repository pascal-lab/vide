#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Cardinalikind {
    Optional,
    Many,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SymbolOrToken {
    Symbol { type_name: String, method_name: String },
    Token { token_name: String, method_name: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Field {
    pub(crate) kind: String,
    pub(crate) cardinalikind: Cardinalikind,
    pub(crate) symbol_or_token: SymbolOrToken,
}

pub(crate) type Fields = std::collections::BTreeMap<String, Field>;

#[derive(Clone, Debug)]
pub(crate) struct Symbol {
    pub(crate) kind: String,
    pub(crate) type_name: String,
    pub(crate) fields: Fields,
}

pub(crate) const TOKEN_REPLACE_PAIR: &[(&str, &str)] = &[
    ("<<<=", "arith_lshift_eq_"),
    (">>>=", "arith_rshift_eq_"),
    ("<<=", "lshift_eq_"),
    (">>=", "rshift_eq_"),
    ("<<<", "arith_lshift_"),
    (">>>", "arith_rshift_"),
    ("===", "eq_eq_eq_"),
    ("!==", "not_eq_eq_"),
    ("==?", "eq_eq_question_"),
    ("&&&", "and_and_and_"),
    ("<->", "less_minus_greater_"),
    ("->", "arrow_"),
    ("+=", "plus_eq_"),
    ("-=", "minus_eq_"),
    ("*=", "star_eq_"),
    ("/=", "slash_eq_"),
    ("%=", "percent_eq_"),
    ("&=", "and_eq_"),
    ("|=", "or_eq_"),
    ("^=", "xor_eq_"),
    ("++", "plus_plus_"),
    ("--", "minus_minus_"),
    ("<<", "lshift_"),
    (">>", "rshift_"),
    ("&&", "and_and_"),
    ("||", "or_or_"),
    ("==", "eq_eq_"),
    ("!=", "not_eq_"),
    ("<=", "less_eq_"),
    (">=", "greater_eq_"),
    ("&=", "and_eq_"),
    ("|=", "or_eq_"),
    ("^=", "xor_eq_"),
    ("->", "arrow_"),
    // ("<=", "non_blocking_assign_"),
    ("::", "coloncolon_"),
    ("~&", "tilde_and_"),
    ("~|", "tilde_or_"),
    ("~^", "tilde_xor_"),
    ("^~", "xor_tilde_"),
    ("**", "star_star_"),
    ("@@", "and_and_"),
    ("$", "dollar_"),
    ("(", "lparen_"),
    (")", "rparen_"),
    ("[", "lbracket_"),
    ("]", "rbracket_"),
    ("{", "lbrace_"),
    ("}", "rbrace_"),
    (",", "comma_"),
    (";", "semicolon_"),
    ("+", "plus_"),
    ("-", "minus_"),
    ("*", "star_"),
    ("/", "slash_"),
    ("%", "percent_"),
    ("&", "and_"),
    ("|", "or_"),
    ("^", "xor_"),
    ("~", "tilde_"),
    ("!", "not_"),
    ("=", "eq_"),
    ("<", "less_"),
    (">", "greater_"),
    (".", "dot_"),
    ("?", "question_"),
    (":", "colon_"),
    ("@", "at_"),
    ("#", "sharp_"),
    ("'", "single_quote_"),
    ("'", ""),
    ("\"", "double_quote_"),
    ("\"", ""),
    ("\\", "backslash_"),
];

pub(crate) fn get_grammar_json() -> serde_json::Value {
    let grammar_json_file =
        sourcegen::project_root().join("crates/tree-sitter-verilog/src/grammar.json");
    let grammar_json = std::fs::read_to_string(grammar_json_file).unwrap();
    serde_json::from_str::<serde_json::Value>(&grammar_json).unwrap()
}
