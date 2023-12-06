use super::ast_src::{
    self, Cardinalikind, Field, Fields, Symbol, SymbolOrToken, TOKEN_REPLACE_PAIR,
};
use inflector::Inflector;
use quote::{format_ident, quote};
use std::collections::{BTreeMap, BTreeSet};
use std::{fs, path::Path};

struct Grammar {
    symbols: Vec<Symbol>,
    tokens: BTreeMap<String, String>,
}

fn mkdir_and_write(file: &Path, contents: &str) -> Result<(), std::io::Error> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(file, contents)
}

pub fn sourcegen_ast() {
    let grammar_json = ast_src::get_grammar_json();
    let grammar = get_fields_and_tokens(&grammar_json);

    let ast_syntax_kinds = generate_syntax_kinds(&grammar);
    let ast_syntax_kinds_file =
        sourcegen::project_root().join("crates/syntax/src/syntax_kind/generated.rs");
    mkdir_and_write(ast_syntax_kinds_file.as_path(), &ast_syntax_kinds)
        .expect("Failed to write syntax kinds file");

    let ast_symbols = generate_nodes(&grammar);
    let ast_symbols_file =
        sourcegen::project_root().join("crates/syntax/src/ast/symbol/generated.rs");
    mkdir_and_write(ast_symbols_file.as_path(), &ast_symbols)
        .expect("Failed to write symbols file");
}

fn generate_nodes(grammar: &Grammar) -> String {
    let symbol_defs = grammar.symbols.iter().map(|symbol| {
        let type_name = format_ident!("{}", symbol.type_name);
        let methods = symbol.fields.values().map(|field| match field.symbol_or_token {
            SymbolOrToken::Symbol { ref method_name, ref type_name } => {
                let type_name = format_ident!("{}", type_name);
                match field.cardinalikind {
                    Cardinalikind::Optional => {
                        let method_name = format_ident!("{}", method_name);
                        quote! {
                            pub fn #method_name(&'a self) -> Option<#type_name<'a>> {
                                support::child(self.syntax())
                            }
                        }
                    }
                    Cardinalikind::Many => {
                        let method_name = format_ident!("{}s", method_name);
                        quote! {
                            pub fn #method_name(&'a self) -> AstChildren<'a, #type_name<'a>> {
                                support::children(self.syntax())
                            }
                        }
                    }
                }
            }
            SymbolOrToken::Token { ref token_name, ref method_name } => {
                let method_name = format_ident!("{}", method_name);
                let token_name = format_ident!("{}", token_name);
                quote! {
                    pub fn #method_name(&'a self) -> Option<SyntaxNode<'a>> {
                        support::token(self.syntax(), syntax_kind::#token_name)
                    }
                }
            }
        });
        quote! {
            #[derive(Debug, Clone, PartialEq, Eq, Hash)]
            pub struct #type_name<'a> {
                syntax: SyntaxNode<'a>
            }

            impl<'a> #type_name<'a> {
                #(#methods)*
            }
        }
    });

    let any_node_defs = grammar.symbols.iter().map(|symbol| {
        let type_name = format_ident!("{}", symbol.type_name);
        let syntax_kind_name = format_ident!("{}", symbol.type_name.to_screaming_snake_case());
        quote! {
            impl<'a> AstNode<'a> for #type_name<'a> {
                fn can_cast(syntax: SyntaxNode<'a>) -> bool {
                    syntax.kind() == syntax_kind::#syntax_kind_name
                }

                fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
                    if Self::can_cast(syntax) {
                        Some(#type_name { syntax })
                    } else {
                        None
                    }
                }

                fn syntax(&self) -> &SyntaxNode {
                    &self.syntax
                }
            }
        }
    });
    let res = quote! {
        #![allow(non_snake_case)]

        use crate::{
            syntax_kind,
            ast::{
                AstNode, SyntaxNode,
                support::{self, AstChildren},
            }
        };

        #(#symbol_defs)*
        #(#any_node_defs)*
    }
    .to_string();
    sourcegen::add_preamble("sourcegen_ast", sourcegen::reformat(res))
        .replace("#[derive", "\n#[derive")
}

fn generate_syntax_kinds(grammar: &Grammar) -> String {
    let symbol_kind_defs = grammar.symbols.iter().map(|symbol| {
        let kind_name = format_ident!("{}", symbol.type_name.to_screaming_snake_case());
        let kind = symbol.kind.to_string();
        quote! {
            pub const #kind_name: &str = #kind;
        }
    });

    let token_kind_defs = grammar.tokens.iter().map(|(token_name, kind)| {
        let token_name = format_ident!("{}", token_name);
        let kind = kind.to_string();
        quote! {
            pub const #token_name: &str = #kind;
        }
    });

    let res = quote! {
        #![allow(non_upper_case_globals)]

        #(#symbol_kind_defs)*
        #(#token_kind_defs)*
    }
    .to_string();
    sourcegen::add_preamble("sourcegen_ast", sourcegen::reformat(res))
}

fn get_fields_and_tokens(grammar_json: &serde_json::Value) -> Grammar {
    let inlined_symbols: BTreeSet<_> = grammar_json["inline"]
        .as_array()
        .unwrap()
        .iter()
        .map(|symbol| symbol.as_str().unwrap().to_string())
        .collect();
    let symbols = grammar_json["rules"].as_object().unwrap().keys();
    let rules = grammar_json["rules"].as_object().unwrap().clone();

    let mut symbol_fields: BTreeMap<String, Fields> = BTreeMap::new();
    let mut tokens: BTreeMap<String, String> = BTreeMap::new();

    let symbols = symbols
        .filter(|kind| !inlined_symbols.contains(*kind))
        .map(|kind| {
            let fields =
                get_rule_fields(kind, &inlined_symbols, &rules, &mut symbol_fields, &mut tokens);
            let method_name = kind.replace('$', "dollar_");
            let type_name = method_name.to_class_case();
            let kind = kind.to_string();
            Symbol { type_name, kind, fields }
        })
        .collect();

    Grammar { symbols, tokens }
}

fn get_rule_fields(
    symbol: &str,
    inlined_symbols: &BTreeSet<String>,
    rules: &serde_json::Map<String, serde_json::Value>,
    symbol_fields: &mut BTreeMap<String, Fields>,
    tokens: &mut BTreeMap<String, String>,
) -> Fields {
    if symbol_fields.contains_key(symbol) {
        return symbol_fields[symbol].clone();
    }
    let fields = get_fields(&rules[symbol], inlined_symbols, rules, symbol_fields, tokens);
    symbol_fields.insert(symbol.to_string(), fields.clone());
    fields
}

fn get_fields(
    rule: &serde_json::Value,
    inlined_symbols: &BTreeSet<String>,
    rules: &serde_json::Map<String, serde_json::Value>,
    symbol_fields: &mut BTreeMap<String, Fields>,
    tokens: &mut BTreeMap<String, String>,
) -> Fields {
    let rule = rule.as_object().unwrap();
    let rule_type = rule["type"].as_str().unwrap();
    match rule_type {
        "SYMBOL" => {
            let kind = rule["name"].as_str().unwrap();
            if inlined_symbols.contains(kind) {
                get_rule_fields(kind, inlined_symbols, rules, symbol_fields, tokens)
            } else {
                let method_name = kind.replace('$', "dollar_");
                let type_name = method_name.to_class_case();
                let mut fields = BTreeMap::new();
                fields.insert(
                    kind.to_string(),
                    Field {
                        kind: kind.to_string(),
                        cardinalikind: Cardinalikind::Optional,
                        symbol_or_token: SymbolOrToken::Symbol { type_name, method_name },
                    },
                );
                fields
            }
        }
        "CHOICE" => {
            let mut fields = Fields::new();
            for alter in rule["members"].as_array().unwrap() {
                let alter_fields = get_fields(alter, inlined_symbols, rules, symbol_fields, tokens);
                for (kind, field) in &alter_fields {
                    if fields.contains_key(kind) {
                        let origin_field = fields.get_mut(kind).unwrap();
                        assert_eq!(origin_field.kind, field.kind);
                        if let Cardinalikind::Many = field.cardinalikind {
                            origin_field.cardinalikind = Cardinalikind::Many;
                        }
                    } else {
                        fields.insert(kind.to_string(), field.clone());
                    }
                }
            }
            fields
        }
        "SEQ" => {
            let mut fields = Fields::new();
            for member in rule["members"].as_array().unwrap() {
                let member_fields =
                    get_fields(member, inlined_symbols, rules, symbol_fields, tokens);
                for (kind, field) in &member_fields {
                    if fields.contains_key(kind) {
                        let origin_field = fields.get_mut(kind).unwrap();
                        assert_eq!(origin_field.kind, field.kind);
                        origin_field.cardinalikind = Cardinalikind::Many;
                    } else {
                        fields.insert(kind.to_string(), field.clone());
                    }
                }
            }
            fields
        }
        "REPEAT" | "REPEAT1" => {
            let content = &rule["content"];
            let mut content_fields =
                get_fields(content, inlined_symbols, rules, symbol_fields, tokens);
            for field in content_fields.values_mut() {
                field.cardinalikind = Cardinalikind::Many;
            }
            content_fields
        }
        "OPTIONAL" => get_fields(&rule["content"], inlined_symbols, rules, symbol_fields, tokens),
        "PREC_LEFT" | "PREC_RIGHT" => {
            get_fields(&rule["content"], inlined_symbols, rules, symbol_fields, tokens)
        }
        "STRING" => {
            let mut fields = Fields::new();
            let kind = rule["value"].as_str().unwrap();
            let string_name = TOKEN_REPLACE_PAIR
                .iter()
                .fold(kind.to_string(), |string_name, (from, to)| string_name.replace(from, to));
            let string_name = string_name.trim_end_matches('_').to_string();
            let string_name = format_ident!("token_{}", string_name).to_string();
            let (method_name, token_name) = match kind {
                "1'b0" => ("token_1_quote_b0".to_string(), "TOKEN_1_QUOTE_b0".to_string()),
                "1'B0" => ("token_1_quote_B0".to_string(), "TOKEN_1_QUOTE_B0".to_string()),
                "1'b1" => ("token_1_quote_b1".to_string(), "TOKEN_1_QUOTE_b1".to_string()),
                "1'B1" => ("token_1_quote_B1".to_string(), "TOKEN_1_QUOTE_B1".to_string()),
                "1'bX" => ("token_1_quote_bX".to_string(), "TOKEN_1_QUOTE_bX".to_string()),
                "1'BX" => ("token_1_quote_BX".to_string(), "TOKEN_1_QUOTE_BX".to_string()),
                "1'bx" => ("token_1_quote_bx".to_string(), "TOKEN_1_QUOTE_bx".to_string()),
                "1'Bx" => ("token_1_quote_Bx".to_string(), "TOKEN_1_QUOTE_Bx".to_string()),
                _ => (string_name.to_string(), string_name.to_screaming_snake_case()),
            };
            fields.insert(
                kind.to_string(),
                Field {
                    kind: kind.to_string(),
                    cardinalikind: Cardinalikind::Optional,
                    symbol_or_token: SymbolOrToken::Token {
                        token_name: token_name.to_string(),
                        method_name: method_name.to_string(),
                    },
                },
            );
            tokens.insert(token_name.to_string(), kind.to_string());
            fields
        }
        "BLANK" => Fields::new(),
        "PATTERN" | "TOKEN" | "IMMEDIATE_TOKEN" => Fields::new(),
        other => todo!("Unknown kind: {}", other),
    }
}
