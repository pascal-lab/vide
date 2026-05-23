use ast::{
    AstNode, CompilationUnit, Expression, LiteralExpression, Member, Name, PrimaryExpression,
};
use expect_test::expect;
use itertools::Itertools;
use utils::test_support::TestDir;

use super::*;

fn get_test_tree() -> SyntaxTree {
    SyntaxTree::from_text("module A(input a); wire x; endmodule;", "source", "")
}

fn get_empty_tree() -> SyntaxTree {
    SyntaxTree::from_text("", "source", "")
}

fn get_multi_module_tree() -> SyntaxTree {
    SyntaxTree::from_text("module A; endmodule; module B; endmodule;", "source", "")
}

fn get_tree_with_trivia() -> SyntaxTree {
    SyntaxTree::from_text(
        r#"
module A();

C #(.l(1)) c();
endmodule

"#,
        "source",
        "",
    )
}

fn get_complex_tree() -> SyntaxTree {
    SyntaxTree::from_text(
        r#"
module A(
  input a,
  /decode,
  output b,
);
endmodule;"#,
        "source",
        "",
    )
}

fn fmt_range(range: &SourceRange) -> String {
    format!("{}..{}", range.start(), range.end())
}

fn dfs(node: SyntaxNode, depth: usize, ans: &mut String) {
    let range = node.range();
    let kind = node.kind();
    let child_count = node.child_count();
    if let Some(range) = range {
        *ans += &format!(
            "{:indent$}{kind:?} {} (cnt: {child_count})\n",
            "",
            fmt_range(&range),
            indent = depth * 2
        );
    } else {
        assert!(kind == SyntaxKind::UNKNOWN || kind.is_list());
        *ans += &format!("{:indent$}{kind:?} (cnt: {child_count})\n", "", indent = depth * 2);
    }

    for i in 0..child_count {
        if let Some(node) = node.child_node(i) {
            dfs(node, depth + 1, ans);
        } else if let Some(tok) = node.child_token(i) {
            tok.trivias_with_loc().for_each(|(loc, trivia)| {
                *ans += &format!(
                    "{:indent$}{:?} {}..{} (trivia)\n",
                    "",
                    trivia.kind(),
                    loc.start,
                    loc.end,
                    indent = (depth + 1) * 2
                );
            });

            if let Some(range) = tok.range() {
                *ans += &format!(
                    "{:indent$}{:?} {}\n",
                    "",
                    tok.kind(),
                    fmt_range(&range),
                    indent = (depth + 1) * 2
                );
            } else {
                *ans += &format!("{:indent$}{:?}\n", "", tok.kind(), indent = (depth + 1) * 2);
            }
        }
    }
}

#[test]
fn parse() {
    let tree = get_test_tree();
    let root = tree.root().unwrap();
    let mut ans = String::new();
    dfs(root, 0, &mut ans);

    let expected = expect![[r#"
        CompilationUnit 0..37 (cnt: 2)
          SyntaxList 0..37 (cnt: 2)
            ModuleDeclaration 0..36 (cnt: 5)
              SyntaxList (cnt: 0)
              ModuleHeader 0..18 (cnt: 7)
                ModuleKeyword 0..6
                Whitespace 6..7 (trivia)
                Identifier 7..8
                SyntaxList (cnt: 0)
                AnsiPortList 8..17 (cnt: 3)
                  OpenParenthesis 8..9
                  SeparatedList 9..16 (cnt: 1)
                    ImplicitAnsiPort 9..16 (cnt: 3)
                      SyntaxList (cnt: 0)
                      VariablePortHeader 9..15 (cnt: 4)
                        InputKeyword 9..14
                        ImplicitType 15..15 (cnt: 3)
                          SyntaxList (cnt: 0)
                          Placeholder 15..15
                      Declarator 15..16 (cnt: 3)
                        Whitespace 14..15 (trivia)
                        Identifier 15..16
                        SyntaxList (cnt: 0)
                  CloseParenthesis 16..17
                Semicolon 17..18
              SyntaxList 19..26 (cnt: 1)
                NetDeclaration 19..26 (cnt: 8)
                  SyntaxList (cnt: 0)
                  Whitespace 18..19 (trivia)
                  WireKeyword 19..23
                  ImplicitType 24..24 (cnt: 3)
                    SyntaxList (cnt: 0)
                    Placeholder 24..24
                  SeparatedList 24..25 (cnt: 1)
                    Declarator 24..25 (cnt: 3)
                      Whitespace 23..24 (trivia)
                      Identifier 24..25
                      SyntaxList (cnt: 0)
                  Semicolon 25..26
              Whitespace 26..27 (trivia)
              EndModuleKeyword 27..36
            EmptyMember 36..37 (cnt: 3)
              SyntaxList (cnt: 0)
              TokenList (cnt: 0)
              Semicolon 36..37
          EndOfFile 37..37
    "#]];
    expected.assert_eq(&ans);
}

#[test]
fn multiple_module() {
    let tree = get_multi_module_tree();
    let root = tree.root().unwrap();
    let mut ans = String::new();
    dfs(root, 0, &mut ans);

    let expected = expect![[r#"
        CompilationUnit 0..41 (cnt: 2)
          SyntaxList 0..41 (cnt: 4)
            ModuleDeclaration 0..19 (cnt: 5)
              SyntaxList (cnt: 0)
              ModuleHeader 0..9 (cnt: 7)
                ModuleKeyword 0..6
                Whitespace 6..7 (trivia)
                Identifier 7..8
                SyntaxList (cnt: 0)
                Semicolon 8..9
              SyntaxList (cnt: 0)
              Whitespace 9..10 (trivia)
              EndModuleKeyword 10..19
            EmptyMember 19..20 (cnt: 3)
              SyntaxList (cnt: 0)
              TokenList (cnt: 0)
              Semicolon 19..20
            ModuleDeclaration 21..40 (cnt: 5)
              SyntaxList (cnt: 0)
              ModuleHeader 21..30 (cnt: 7)
                Whitespace 20..21 (trivia)
                ModuleKeyword 21..27
                Whitespace 27..28 (trivia)
                Identifier 28..29
                SyntaxList (cnt: 0)
                Semicolon 29..30
              SyntaxList (cnt: 0)
              Whitespace 30..31 (trivia)
              EndModuleKeyword 31..40
            EmptyMember 40..41 (cnt: 3)
              SyntaxList (cnt: 0)
              TokenList (cnt: 0)
              Semicolon 40..41
          EndOfFile 41..41
    "#]];
    expected.assert_eq(&ans);
}

#[test]
fn no_location() {
    let tree = get_empty_tree();
    let root = tree.root().unwrap();
    let node = root.child_node(0).unwrap();
    let range = node.range();
    assert!(range.is_none());
}

#[test]
fn kind() {
    let tree = get_test_tree();
    let root = tree.root().unwrap();

    assert_eq!(root.kind(), SyntaxKind::COMPILATION_UNIT);
}

#[test]
fn diagnostic_severity_from_raw() {
    assert_eq!(DiagnosticSeverity::from_raw(0), DiagnosticSeverity::Ignored);
    assert_eq!(DiagnosticSeverity::from_raw(1), DiagnosticSeverity::Note);
    assert_eq!(DiagnosticSeverity::from_raw(2), DiagnosticSeverity::Warning);
    assert_eq!(DiagnosticSeverity::from_raw(3), DiagnosticSeverity::Error);
    assert_eq!(DiagnosticSeverity::from_raw(4), DiagnosticSeverity::Fatal);
    assert_eq!(DiagnosticSeverity::from_raw(255), DiagnosticSeverity::Fatal);
}

#[test]
fn keyword_tables_are_available_from_rust() {
    let sv_keywords = SyntaxToken::keyword_table_for_version("1800-2023");
    let verilog_keywords = SyntaxToken::verilog_2005_keywords();

    let sv_keywords_sorted = sv_keywords.iter().sorted().collect_vec();
    let verilog_keywords_sorted = verilog_keywords.iter().sorted().collect_vec();

    assert!(!sv_keywords.is_empty(), "expected SystemVerilog keyword table to be non-empty");
    assert!(!verilog_keywords.is_empty(), "expected Verilog-2005 keyword table to be non-empty");

    assert!(
        sv_keywords.iter().any(|keyword| keyword == "module"),
        "expected `module` in SystemVerilog keyword table: {sv_keywords_sorted:?}"
    );
    assert!(
        sv_keywords.iter().any(|keyword| keyword == "endmodule"),
        "expected `endmodule` in SystemVerilog keyword table: {sv_keywords_sorted:?}"
    );
    assert!(
        sv_keywords.iter().any(|keyword| keyword == "interface"),
        "expected `interface` in SystemVerilog keyword table: {sv_keywords_sorted:?}"
    );

    assert!(
        verilog_keywords.iter().any(|keyword| keyword == "module"),
        "expected `module` in Verilog-2005 keyword table: {verilog_keywords_sorted:?}"
    );
    assert!(
        verilog_keywords.iter().any(|keyword| keyword == "endmodule"),
        "expected `endmodule` in Verilog-2005 keyword table: {verilog_keywords_sorted:?}"
    );
    assert!(
        verilog_keywords.iter().all(|keyword| keyword != "interface"),
        "did not expect `interface` in Verilog-2005 keyword table: {verilog_keywords_sorted:?}"
    );

    assert_eq!(SyntaxToken::keyword_kind_for_version("1364-2005", "input"), Token![input]);
    assert_eq!(
        SyntaxToken::keyword_kind_for_version("1364-2005", "not_a_keyword"),
        TokenKind::UNKNOWN
    );
}

#[test]
fn syntax_kind_all_exposes_directive_kinds() {
    let directives = SyntaxKind::ALL
        .iter()
        .filter_map(|kind| {
            let text = SyntaxToken::directive_text(*kind);
            let text = text.trim_start_matches('`');
            (!text.is_empty()).then_some(text.to_string())
        })
        .collect_vec();

    assert!(directives.iter().any(|directive| directive == "define"));
    assert!(directives.iter().any(|directive| directive == "include"));
    assert!(directives.iter().any(|directive| directive == "timescale"));
}

#[test]
fn syntax_facts_are_available_from_rust() {
    assert!(SyntaxFacts::is_possible_statement(Token![if]));
    assert!(!SyntaxFacts::is_possible_statement(Token![module]));
    assert!(!SyntaxFacts::is_possible_expression(Token![module]));
    assert!(SyntaxFacts::is_possible_data_type(Token![integer]));
    assert!(SyntaxFacts::is_possible_argument(Token![.]));
    assert!(SyntaxFacts::is_possible_param_assignment(Token![.]));
    assert!(SyntaxFacts::is_possible_port_connection(Token![.]));
    assert!(SyntaxFacts::is_possible_ansi_port(Token![input]));
    assert!(SyntaxFacts::is_possible_non_ansi_port(Token![,]));
    assert!(SyntaxFacts::is_possible_function_port(Token![default]));
    assert!(SyntaxFacts::is_possible_parameter(Token![parameter]));
    assert!(SyntaxFacts::is_gate_type(Token![and]));
    assert!(SyntaxFacts::is_gate_type(Token![bufif0]));
    assert!(!SyntaxFacts::is_gate_type(Token![module]));
    assert!(SemanticFacts::is_edge_kind(Token![posedge]));
    assert!(SemanticFacts::is_edge_kind(Token![edge]));
    assert!(!SemanticFacts::is_edge_kind(Token![module]));
    assert!(SyntaxFacts::is_port_direction(Token![output]));
    assert!(SyntaxFacts::is_net_type(Token![wire]));
    assert_eq!(SyntaxFacts::get_integer_type(Token![integer]), SyntaxKind::INTEGER_TYPE);
    assert_eq!(SyntaxFacts::get_keyword_type(Token![event]), SyntaxKind::EVENT_TYPE);
    assert_eq!(SyntaxFacts::get_procedural_block_kind(Token![always]), SyntaxKind::ALWAYS_BLOCK);
    assert_eq!(
        SyntaxFacts::get_module_declaration_kind(Token![module]),
        SyntaxKind::MODULE_DECLARATION
    );
    assert!(SyntaxFacts::is_possible_member_kind(Token![assign], SyntaxKind::CONTINUOUS_ASSIGN));
    assert!(SyntaxFacts::is_possible_member_kind(Token![begin], SyntaxKind::GENERATE_BLOCK));
    assert!(SyntaxFacts::is_possible_member_kind(Token![case], SyntaxKind::CASE_GENERATE));
    assert!(!SyntaxFacts::is_possible_member_kind(Token![casex], SyntaxKind::CASE_GENERATE));
    assert_eq!(
        SyntaxFacts::get_block_item_declaration_kind(Token![localparam]),
        SyntaxKind::PARAMETER_DECLARATION_STATEMENT
    );
    assert_eq!(
        SyntaxFacts::get_library_map_member_kind(Token![library]),
        SyntaxKind::LIBRARY_DECLARATION
    );
    assert_eq!(
        SyntaxFacts::get_specify_item_kind(Token![pulsestyle_ondetect]),
        SyntaxKind::PULSE_STYLE_DECLARATION
    );
    assert_eq!(
        SyntaxFacts::get_config_header_item_kind(Token![design]),
        SyntaxKind::CONFIG_DECLARATION
    );
    assert_eq!(SyntaxFacts::get_config_rule_kind(Token![default]), SyntaxKind::DEFAULT_CONFIG_RULE);

    let module_keywords = SyntaxFacts::keyword_candidates_for_context(
        "1364-2005",
        SyntaxKeywordContext::ModuleMember,
    );
    assert!(module_keywords.iter().any(|keyword| keyword == "assign"));
    assert!(module_keywords.iter().any(|keyword| keyword == "begin"));
    assert!(!module_keywords.iter().any(|keyword| keyword == "while"));

    let library_keywords = SyntaxFacts::keyword_candidates_for_context(
        "1364-2005",
        SyntaxKeywordContext::LibraryMapMember,
    );
    assert!(library_keywords.iter().any(|keyword| keyword == "library"));
    assert!(library_keywords.iter().any(|keyword| keyword == "include"));
    assert!(!library_keywords.iter().any(|keyword| keyword == "module"));

    let generate_keywords = SyntaxFacts::keyword_candidates_for_context(
        "1364-2005",
        SyntaxKeywordContext::GenerateMember,
    );
    assert!(generate_keywords.iter().any(|keyword| keyword == "case"));
    assert!(!generate_keywords.iter().any(|keyword| keyword == "casex"));

    let system_functions = Compilation::system_function_names();
    assert!(system_functions.iter().any(|name| name == "$clog2"));
    assert!(system_functions.iter().any(|name| name == "$bits"));
    assert!(!system_functions.iter().any(|name| name == "$display"));

    let system_tasks = Compilation::system_task_names();
    assert!(system_tasks.iter().any(|name| name == "$display"));
    assert!(!system_tasks.iter().any(|name| name == "$clog2"));

    assert!(SyntaxFacts::is_allowed_in_compilation_unit(SyntaxKind::MODULE_DECLARATION));
    let _ = SyntaxFacts::is_allowed_in_generate(SyntaxKind::INITIAL_BLOCK);
    assert!(SyntaxFacts::is_allowed_in_module(SyntaxKind::INITIAL_BLOCK));
    let _ = SyntaxFacts::is_allowed_in_interface(SyntaxKind::INITIAL_BLOCK);
    let _ = SyntaxFacts::is_allowed_in_program(SyntaxKind::INITIAL_BLOCK);
    assert!(!SyntaxFacts::is_allowed_in_package(SyntaxKind::INITIAL_BLOCK));
}

#[test]
fn test_partial_eq_syntax_node() {
    let tree = get_complex_tree();
    let root = tree.root().unwrap();
    let node = root.child_node(0).unwrap();
    let node2 = root.child_node(0).unwrap();
    assert!(node == node2);
}

#[test]
fn parse_complex() {
    let tree = get_complex_tree();
    let root = tree.root().unwrap();
    let mut ans = String::new();
    dfs(root, 0, &mut ans);

    let expected = expect![[r#"
        CompilationUnit 1..58 (cnt: 2)
          SyntaxList 1..58 (cnt: 2)
            ModuleDeclaration 1..57 (cnt: 5)
              SyntaxList (cnt: 0)
              ModuleHeader 1..47 (cnt: 7)
                EndOfLine 0..1 (trivia)
                ModuleKeyword 1..7
                Whitespace 7..8 (trivia)
                Identifier 8..9
                SyntaxList (cnt: 0)
                AnsiPortList 9..46 (cnt: 3)
                  OpenParenthesis 9..10
                  SeparatedList 13..44 (cnt: 6)
                    ImplicitAnsiPort 13..20 (cnt: 3)
                      SyntaxList (cnt: 0)
                      VariablePortHeader 13..19 (cnt: 4)
                        EndOfLine 10..11 (trivia)
                        Whitespace 11..13 (trivia)
                        InputKeyword 13..18
                        ImplicitType 19..19 (cnt: 3)
                          SyntaxList (cnt: 0)
                          Placeholder 19..19
                      Declarator 19..20 (cnt: 3)
                        Whitespace 18..19 (trivia)
                        Identifier 19..20
                        SyntaxList (cnt: 0)
                    Comma 20..21
                    ImplicitAnsiPort 24..21 (cnt: 3)
                      SyntaxList (cnt: 0)
                      VariablePortHeader 24..24 (cnt: 4)
                        ImplicitType 24..24 (cnt: 3)
                          SyntaxList (cnt: 0)
                          Placeholder 24..24
                      Declarator 21..21 (cnt: 3)
                        Identifier 21..21
                        SyntaxList (cnt: 0)
                    SkippedTokens 21..21 (trivia)
                    Comma 31..32
                    ImplicitAnsiPort 35..43 (cnt: 3)
                      SyntaxList (cnt: 0)
                      VariablePortHeader 35..42 (cnt: 4)
                        EndOfLine 32..33 (trivia)
                        Whitespace 33..35 (trivia)
                        OutputKeyword 35..41
                        ImplicitType 42..42 (cnt: 3)
                          SyntaxList (cnt: 0)
                          Placeholder 42..42
                      Declarator 42..43 (cnt: 3)
                        Whitespace 41..42 (trivia)
                        Identifier 42..43
                        SyntaxList (cnt: 0)
                    Comma 43..44
                  EndOfLine 44..45 (trivia)
                  CloseParenthesis 45..46
                Semicolon 46..47
              SyntaxList (cnt: 0)
              EndOfLine 47..48 (trivia)
              EndModuleKeyword 48..57
            EmptyMember 57..58 (cnt: 3)
              SyntaxList (cnt: 0)
              TokenList (cnt: 0)
              Semicolon 57..58
          EndOfFile 58..58
    "#]];
    expected.assert_eq(&ans);
}

#[test]
fn macro_expanded_declarator_range_maps_to_expansion_site() {
    let text = r#"`include "include/code_action_defs.vh"

module ca_leaf #(
    parameter WIDTH = `CA_WIDTH,
    parameter RESET_VALUE = 0
) ();
endmodule
"#;
    let options = SyntaxTreeOptions {
        include_buffers: vec![SyntaxTreeBuffer {
            path: String::from("sample/include/code_action_defs.vh"),
            text: String::from("`define CA_WIDTH 8\n"),
        }],
        include_paths: vec![String::from("sample")],
        predefines: Vec::new(),
        ..SyntaxTreeOptions::default()
    };
    let tree = SyntaxTree::from_text_with_options(
        text,
        "sample/rtl/code_action_targets.v",
        "sample/rtl/code_action_targets.v",
        &options,
    );
    let root = tree.root().unwrap();
    let declarator = root
        .node_preorder()
        .filter_map(|event| match event {
            WalkEvent::Enter(node) => ast::Declarator::cast(node),
            WalkEvent::Leave(_) => None,
        })
        .find(|decl| decl.name().is_some_and(|name| name.value_text().as_bytes() == b"WIDTH"))
        .expect("WIDTH declarator should be present");

    let range = declarator.syntax().range().expect("declarator should have a range");
    let start = text.find("WIDTH").unwrap();
    let end = text.find("`CA_WIDTH").unwrap() + "`CA_WIDTH".len();

    assert!(range.is_single_buffer());
    assert_eq!(range.start(), start);
    assert_eq!(range.end(), end);
}

#[test]
fn test_literals() {
    let tree = SyntaxTree::from_text(
        r#"
module A();
  wire y = 3s;
  wire x = 7'b0010xx10;
  wire z = 12;
  wire p = 'x;
  wire empty = ;
endmodule;"#,
        "source",
        "",
    );
    let root = tree.root().unwrap();

    let unit = CompilationUnit::cast(root).unwrap();
    let module = match unit.members().children().next().unwrap() {
        Member::ModuleDeclaration(module) => module,
        _ => unreachable!("expected module declaration"),
    };
    let mut literals = module.members().children().map(|member| {
        let Member::NetDeclaration(decl) = member else {
            unreachable!("expected net declaration");
        };
        let decl = decl.declarators().children().next().unwrap();
        decl.initializer().unwrap().expr()
    });

    let Expression::PrimaryExpression(PrimaryExpression::LiteralExpression(
        LiteralExpression::TimeLiteralExpression(time_lit),
    )) = literals.next().unwrap()
    else {
        unreachable!("expected time literal");
    };
    let time_tok = time_lit.child_token(0).unwrap();
    assert_eq!(time_tok.time_unit(), Some(TimeUnit::Seconds));
    assert_eq!(time_tok.base(), None);
    assert!((time_tok.real().unwrap() - 3.0).abs() < f64::EPSILON);

    let Expression::PrimaryExpression(PrimaryExpression::IntegerVectorExpression(vec_lit)) =
        literals.next().unwrap()
    else {
        unreachable!("expected integer vector literal");
    };
    assert_eq!(vec_lit.size().unwrap().int().unwrap().get_single_word(), Some(7));
    assert_eq!(vec_lit.base().unwrap().base(), Some(LiteralBase::Bin));
    assert_eq!(vec_lit.value().unwrap().int().unwrap().serialize(2), "10xx10");

    let Expression::PrimaryExpression(PrimaryExpression::LiteralExpression(
        LiteralExpression::IntegerLiteralExpression(int_lit),
    )) = literals.next().unwrap()
    else {
        unreachable!("expected integer literal");
    };
    let int_tok = int_lit.child_token(0).unwrap();
    let svint = int_tok.int().unwrap();
    assert_eq!(svint.to_string(), "12");
    assert_eq!(svint.get_single_word(), Some(12));

    let svint2 = svint.clone();
    assert_eq!(svint, svint2);

    let Expression::PrimaryExpression(PrimaryExpression::LiteralExpression(
        LiteralExpression::UnbasedUnsizedLiteralExpression(unbased_lit),
    )) = literals.next().unwrap()
    else {
        unreachable!("expected unbased unsized literal");
    };
    let unbased_tok = unbased_lit.child_token(0).unwrap();
    assert_eq!(unbased_tok.bits().unwrap().bit(), Bit::X);

    let Expression::Name(Name::IdentifierName(name)) = literals.next().unwrap() else {
        unreachable!("expected identifier");
    };
    assert!(name.identifier().unwrap().value_text().to_string().is_empty());
}

#[test]
fn test_trivia() {
    let tree = get_tree_with_trivia();
    let root = tree.root().unwrap();
    let mut ans = String::new();
    dfs(root, 0, &mut ans);

    let expected = expect![[r#"
        CompilationUnit 1..41 (cnt: 2)
          SyntaxList 1..39 (cnt: 1)
            ModuleDeclaration 1..39 (cnt: 5)
              SyntaxList (cnt: 0)
              ModuleHeader 1..12 (cnt: 7)
                EndOfLine 0..1 (trivia)
                ModuleKeyword 1..7
                Whitespace 7..8 (trivia)
                Identifier 8..9
                SyntaxList (cnt: 0)
                AnsiPortList 9..11 (cnt: 3)
                  OpenParenthesis 9..10
                  SeparatedList (cnt: 0)
                  CloseParenthesis 10..11
                Semicolon 11..12
              SyntaxList 14..29 (cnt: 1)
                HierarchyInstantiation 14..29 (cnt: 5)
                  SyntaxList (cnt: 0)
                  EndOfLine 12..13 (trivia)
                  EndOfLine 13..14 (trivia)
                  Identifier 14..15
                  ParameterValueAssignment 16..24 (cnt: 4)
                    Whitespace 15..16 (trivia)
                    Hash 16..17
                    OpenParenthesis 17..18
                    SeparatedList 18..23 (cnt: 1)
                      NamedParamAssignment 18..23 (cnt: 5)
                        Dot 18..19
                        Identifier 19..20
                        OpenParenthesis 20..21
                        IntegerLiteralExpression 21..22 (cnt: 1)
                          IntegerLiteral 21..22
                        CloseParenthesis 22..23
                    CloseParenthesis 23..24
                  SeparatedList 25..28 (cnt: 1)
                    HierarchicalInstance 25..28 (cnt: 4)
                      InstanceName 25..26 (cnt: 2)
                        Whitespace 24..25 (trivia)
                        Identifier 25..26
                        SyntaxList (cnt: 0)
                      OpenParenthesis 26..27
                      SeparatedList (cnt: 0)
                      CloseParenthesis 27..28
                  Semicolon 28..29
              EndOfLine 29..30 (trivia)
              EndModuleKeyword 30..39
          EndOfLine 39..40 (trivia)
          EndOfLine 40..41 (trivia)
          EndOfFile 41..41
    "#]];
    expected.assert_eq(&ans);
}

#[test]
fn test_compilation() {
    let mut compilation = Compilation::new();
    let tree = get_test_tree();
    compilation.add_syntax_tree(tree);
}

#[test]
fn syntax_tree_diagnostics_are_built_in_cpp() {
    let tree = SyntaxTree::from_text("module A( input a; endmodule", "source", "");
    let diagnostics = tree.diagnostics();

    assert!(!diagnostics.is_empty(), "expected parse diagnostics");

    let diag = &diagnostics[0];
    assert_eq!(diag.severity, DiagnosticSeverity::Error);
    assert!(!diag.message.is_empty(), "expected formatted diagnostic message");
    assert!(diag.location.is_some(), "expected location in diagnostic");
}

#[test]
fn parser_expected_syntax_reports_member_positions() {
    let text = "module m;\n  \nendmodule\n";
    let offset = text.find("endmodule").unwrap();
    let expected = SyntaxTree::expected_syntax_at_offset(text, "source", "", offset);

    assert!(
        expected.iter().any(|item| {
            item.name == "ExpectedMember"
                && item.keyword_context == Some(SyntaxKeywordContext::ModuleMember)
                && item.location == Some(offset)
        }),
        "expected parser member expectation at offset, got {expected:?}"
    );
}

#[test]
fn parser_expected_syntax_reports_statement_positions() {
    let text = "module m; initial begin\n  \nend endmodule\n";
    let offset = text.find("end endmodule").unwrap();
    let expected = SyntaxTree::expected_syntax_at_offset(text, "source", "", offset);

    assert!(
        expected.iter().any(|item| {
            item.name == "ExpectedStatement"
                && item.keyword_context == Some(SyntaxKeywordContext::BlockItem)
                && item.location == Some(offset)
        }),
        "expected parser statement expectation at offset, got {expected:?}"
    );
}

#[test]
fn parser_expected_syntax_reports_list_item_positions() {
    let text = "module m #(\n  \n) (); endmodule\n";
    let offset = text.find(") ();").unwrap();
    let expected = SyntaxTree::expected_syntax_at_offset(text, "source", "", offset);

    assert!(
        expected.iter().any(|item| {
            item.name == "ExpectedParameterPort"
                && item.keyword_context == Some(SyntaxKeywordContext::ParameterPortListItem)
                && item.location == Some(offset)
        }),
        "expected parser parameter port expectation at offset, got {expected:?}"
    );
}

#[test]
fn parser_expected_syntax_reports_expression_positions() {
    let text = "module m; logic [7:0] lhs = ; endmodule\n";
    let offset = text.find("; endmodule").unwrap();
    let expected = SyntaxTree::expected_syntax_at_offset(text, "source", "", offset);

    assert!(
        expected
            .iter()
            .any(|item| item.name == "ExpectedExpression" && item.location == Some(offset)),
        "expected parser expression expectation at offset, got {expected:?}"
    );
}

#[test]
fn directive_at_offset_reports_lexer_range_and_prefix() {
    let text = "`de/*cursor*/fine FOO 1\nmodule m; endmodule\n";
    let offset = text.find("/*cursor*/").unwrap();
    let text = text.replace("/*cursor*/", "");
    let directive = SyntaxTree::directive_at_offset(&text, "source", "", offset)
        .expect("expected directive token at cursor");

    assert_eq!(directive.replacement, 1..7);
    assert_eq!(directive.prefix, "de");
    assert_eq!(directive.token_kind, TokenKind::DIRECTIVE);
    assert_eq!(directive.directive_kind, Some(SyntaxKind::DEFINE_DIRECTIVE));
}

#[test]
fn directive_at_offset_reports_macro_usage_prefix() {
    let text = "module m; initial `de/*cursor*/; endmodule\n";
    let offset = text.find("/*cursor*/").unwrap();
    let text = text.replace("/*cursor*/", "");
    let directive = SyntaxTree::directive_at_offset(&text, "source", "", offset)
        .expect("expected directive-like token at cursor");

    assert_eq!(directive.replacement, 19..21);
    assert_eq!(directive.prefix, "de");
    assert_eq!(directive.token_kind, TokenKind::DIRECTIVE);
    assert_eq!(directive.directive_kind, Some(SyntaxKind::MACRO_USAGE));
}

#[test]
fn directive_at_offset_ignores_strings_and_comments() {
    for text in ["\"`de/*cursor*/fine\"", "// `de/*cursor*/fine\nmodule m; endmodule\n"] {
        let offset = text.find("/*cursor*/").unwrap();
        let text = text.replace("/*cursor*/", "");
        assert_eq!(SyntaxTree::directive_at_offset(&text, "source", "", offset), None);
    }
}

#[test]
fn token_word_at_offset_reports_identifier_range_and_prefix() {
    let text = "lib/*cursor*/\n";
    let offset = text.find("/*cursor*/").unwrap();
    let text = text.replace("/*cursor*/", "");
    let word = SyntaxTree::token_word_at_offset(&text, "source", "", offset)
        .expect("expected identifier token at cursor");

    assert_eq!(word.replacement, 0..3);
    assert_eq!(word.prefix, "lib");
    assert_eq!(word.token_kind, TokenKind::IDENTIFIER);
}

#[test]
fn token_word_at_offset_ignores_non_identifier_tokens() {
    let text = "4/*cursor*/2\n";
    let offset = text.find("/*cursor*/").unwrap();
    let text = text.replace("/*cursor*/", "");
    assert_eq!(SyntaxTree::token_word_at_offset(&text, "source", "", offset), None);
}

#[test]
fn compilation_semantic_diagnostics_are_built_in_cpp() {
    let mut compilation = Compilation::new();
    compilation.add_syntax_tree(SyntaxTree::from_text(
        r#"
module A;
  wire x;
  logic x;
endmodule
"#,
        "source",
        "",
    ));

    let diagnostics = compilation.semantic_diagnostics();
    assert!(!diagnostics.is_empty(), "expected semantic diagnostics");

    let diag = &diagnostics[0];
    assert!(matches!(diag.severity, DiagnosticSeverity::Error | DiagnosticSeverity::Fatal));
    assert!(!diag.message.is_empty(), "expected formatted diagnostic message");
    assert!(diag.location.is_some(), "expected location in diagnostic");
}

#[test]
fn syntax_tree_options_apply_predefines() {
    let tree = SyntaxTree::from_text_with_options(
        r#"
`ifndef HAVE_PREDEFINE
module broken(
`endif
module ok;
endmodule
"#,
        "source",
        "",
        &SyntaxTreeOptions {
            predefines: vec!["HAVE_PREDEFINE".to_owned()],
            ..SyntaxTreeOptions::default()
        },
    );

    assert_eq!(tree.diagnostics(), Vec::new());
}

#[test]
fn syntax_tree_options_apply_include_paths() {
    let dir = TestDir::new("slang-include");
    dir.write("defs.svh", "`define HAVE_HEADER\n");

    let tree = SyntaxTree::from_text_with_options(
        r#"
`include "defs.svh"
`ifndef HAVE_HEADER
module broken(
`endif
module ok;
endmodule
"#,
        "source",
        "",
        &SyntaxTreeOptions {
            include_paths: vec![dir.path().to_string()],
            ..SyntaxTreeOptions::default()
        },
    );

    let diagnostics = tree.diagnostics();

    assert_eq!(diagnostics, Vec::new());
}

#[test]
fn syntax_tree_options_apply_in_memory_include_buffers() {
    let dir = TestDir::new("slang-include-buffer");
    let rtl_dir = dir.create_dir_all("rtl");
    let include_dir = dir.create_dir_all("include");
    let header_path = dir.write("include/defs.svh", "");
    let source_path = rtl_dir.join("top.sv").to_string();

    let tree = SyntaxTree::from_text_with_options(
        r#"
`include "defs.svh"
`ifndef HAVE_HEADER
module broken(
`endif
module ok;
endmodule
"#,
        "source",
        &source_path,
        &SyntaxTreeOptions {
            include_paths: vec![include_dir.to_string()],
            include_buffers: vec![SyntaxTreeBuffer {
                path: header_path.to_string(),
                text: "`define HAVE_HEADER\n".to_owned(),
            }],
            ..SyntaxTreeOptions::default()
        },
    );

    let diagnostics = tree.diagnostics();

    assert_eq!(diagnostics, Vec::new());
}

#[cfg(windows)]
#[test]
fn syntax_tree_options_deduplicate_equivalent_include_buffer_aliases() {
    let dir = TestDir::new("slang-include-buffer-aliases");
    let rtl_dir = dir.create_dir_all("rtl");
    let include_dir = dir.create_dir_all("include");
    let header_path = dir.write("include/defs.svh", "");
    let source_path = rtl_dir.join("top.sv").to_string().replace('\\', "/");
    let header_path = header_path.to_string().replace('\\', "/");
    let include_dir = include_dir.to_string().replace('\\', "/");

    let tree = SyntaxTree::from_text_with_options(
        r#"
`include "defs.svh"
`ifndef HAVE_HEADER
module broken(
`endif
module ok;
endmodule
"#,
        "source",
        &source_path,
        &SyntaxTreeOptions {
            include_paths: vec![include_dir],
            include_buffers: vec![SyntaxTreeBuffer {
                path: header_path,
                text: "`define HAVE_HEADER\n".to_owned(),
            }],
            ..SyntaxTreeOptions::default()
        },
    );

    assert_eq!(tree.diagnostics(), Vec::new());
}

#[test]
fn syntax_tree_options_apply_relative_in_memory_include_buffers() {
    let tree = SyntaxTree::from_text_with_options(
        r#"
`include "include/defs.svh"
`ifndef HAVE_HEADER
module broken(
`endif
module ok;
endmodule
"#,
        "sample/rtl/top.sv",
        "sample/rtl/top.sv",
        &SyntaxTreeOptions {
            include_paths: vec![String::from("sample")],
            include_buffers: vec![SyntaxTreeBuffer {
                path: String::from("sample/include/defs.svh"),
                text: String::from("`define HAVE_HEADER\n"),
            }],
            ..SyntaxTreeOptions::default()
        },
    );

    assert_eq!(tree.diagnostics(), Vec::new());
}
