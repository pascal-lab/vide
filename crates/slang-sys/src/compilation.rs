mod ffi;

use std::pin::Pin;

use cxx::UniquePtr;

use crate::{
    diagnostic::SyntaxDiagnostic,
    syntax::{SyntaxTree, SyntaxTreeOptions},
};

pub struct Compilation {
    raw: UniquePtr<ffi::Compilation>,
}

impl Default for Compilation {
    fn default() -> Self {
        Self::new()
    }
}

impl Compilation {
    pub fn new() -> Self {
        Self { raw: ffi::new_compilation(Vec::new()) }
    }

    pub fn new_with_top_modules(top_modules: &[String]) -> Self {
        Self { raw: ffi::new_compilation(top_modules.to_vec()) }
    }

    pub fn add_syntax_tree(&mut self, tree: SyntaxTree) {
        ffi::add_syntax_tree(self.raw_pin(), tree.into_raw());
    }

    pub fn parse_syntax_tree_from_text(
        &mut self,
        text: &str,
        name: &str,
        path: &str,
        options: &SyntaxTreeOptions,
    ) -> SyntaxTree {
        SyntaxTree::from_raw(ffi::parse_syntax_tree_from_text(
            self.raw_pin(),
            text,
            name,
            path,
            options.predefines.clone(),
            options.include_paths.clone(),
            options.include_buffers.iter().map(|buffer| buffer.path.clone()).collect(),
            options.include_buffers.iter().map(|buffer| buffer.text.clone()).collect(),
            options.expand_includes,
            options.collect_expected_syntax,
        ))
    }

    pub fn parse_library_map_syntax_tree_from_text(
        &mut self,
        text: &str,
        name: &str,
        path: &str,
    ) -> SyntaxTree {
        SyntaxTree::from_raw(ffi::parse_library_map_syntax_tree_from_text(
            self.raw_pin(), text, name, path,
        ))
    }

    pub fn parse_diagnostics_with_options(
        &self,
        warning_options: &[String],
    ) -> Vec<SyntaxDiagnostic> {
        ffi::parse_diagnostics(&self.raw, warning_options.to_vec())
            .into_iter()
            .map(SyntaxDiagnostic::from_raw)
            .collect()
    }

    pub fn semantic_diagnostics_with_options(
        &self,
        warning_options: &[String],
    ) -> Vec<SyntaxDiagnostic> {
        ffi::semantic_diagnostics(&self.raw, warning_options.to_vec())
            .into_iter()
            .map(SyntaxDiagnostic::from_raw)
            .collect()
    }

    pub fn system_function_names() -> Vec<String> {
        ffi::system_function_names()
    }

    pub fn system_task_names() -> Vec<String> {
        ffi::system_task_names()
    }

    fn raw_pin(&mut self) -> Pin<&mut ffi::Compilation> {
        self.raw.as_mut().expect("Slang compilation unexpectedly null")
    }
}
