mod syntax_sourcegen;

fn main() {
    println!("cargo:rerun-if-changed=./build.rs");
    println!("cargo:rerun-if-changed=./syntax_sourcegen");
    println!("cargo:rerun-if-changed=../tree-sitter-verilog/src/grammar.json");
    syntax_sourcegen::sourcegen_ast::sourcegen_ast();
}
