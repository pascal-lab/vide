use std::{env, fs, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let lexer_path = manifest_dir.join("../slang/source/parsing/LexerFacts.cpp");
    println!("cargo:rerun-if-changed={}", lexer_path.display());
    println!("cargo:rerun-if-changed={}", manifest_dir.join("build.rs").display());

    let mut keywords = slang::verilog_2005_keywords();
    keywords.sort();
    keywords.dedup();
    let output = render_toml(&keywords);

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let out_path = out_dir.join("keywords.generated.toml");
    fs::write(&out_path, output).expect("write keywords.generated.toml");
}

fn render_toml(keywords: &[String]) -> String {
    let mut out = String::from(
        "# Generated from crates/slang/source/parsing/LexerFacts.cpp\n\
         # Verilog-2005 keyword set (1364-1995 + 1364-2001 + 1364-2005).\n",
    );

    for keyword in keywords {
        out.push_str("\n[[module_item]]\n");
        out.push_str(&format!("label = \"{keyword}\"\n"));
        out.push_str(&format!("plain = \"{keyword}\"\n"));
        out.push_str("kind = \"keyword\"\n");
    }

    out
}
