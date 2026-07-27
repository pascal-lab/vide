//! This file defines the typed AST nodes that are generated from slang's syntax
//! tree.
mod slang_ast {
    include!(concat!(env!("OUT_DIR"), "/ast.rs"));
}
pub use slang_ast::*;
