use preproc::directive_index::{MacroInclude, PreprocFileIndex};
use syntax::{
    LexedTokenAtOffset, ParserExpectedSyntax, SyntaxDiagnostic, SyntaxTree, SyntaxTreeBufferIds,
    SyntaxTreeOptions,
};

pub fn parse_syntax(text: &str, name: &str, path: &str) -> SyntaxTree {
    slang_adapter::parse_syntax(text, name, path).expect("slang parse should produce a root")
}

pub fn parse_syntax_with_options(
    text: &str,
    name: &str,
    path: &str,
    options: &SyntaxTreeOptions,
) -> SyntaxTree {
    slang_adapter::parse_syntax_with_options(text, name, path, options)
        .expect("slang parse should produce a root")
}

pub struct ParsedSyntax {
    pub tree: SyntaxTree,
    pub diagnostics: Vec<SyntaxDiagnostic>,
}

pub fn parse_syntax_with_diagnostics(
    text: &str,
    name: &str,
    path: &str,
    options: &SyntaxTreeOptions,
    warning_options: &[String],
) -> ParsedSyntax {
    let parsed =
        slang_adapter::parse_syntax_with_diagnostics(text, name, path, options, warning_options)
            .expect("slang parse should produce a root");
    ParsedSyntax { tree: parsed.tree, diagnostics: parsed.diagnostics }
}

pub fn parse_library_map_syntax(text: &str, name: &str, path: &str) -> SyntaxTree {
    slang_adapter::parse_library_map_syntax(text, name, path)
        .expect("slang parse should produce a root")
}

pub fn parse_library_map_syntax_with_diagnostics(
    text: &str,
    name: &str,
    path: &str,
    warning_options: &[String],
) -> ParsedSyntax {
    let parsed =
        slang_adapter::parse_library_map_syntax_with_diagnostics(text, name, path, warning_options)
            .expect("slang parse should produce a root");
    ParsedSyntax { tree: parsed.tree, diagnostics: parsed.diagnostics }
}

pub fn parse_diagnostics_with_options(
    text: &str,
    name: &str,
    path: &str,
    options: &SyntaxTreeOptions,
    warning_options: &[String],
) -> Vec<SyntaxDiagnostic> {
    slang_adapter::parse_diagnostics_with_options(text, name, path, options, warning_options)
}

pub fn expected_syntax_at_offset(
    text: &str,
    name: &str,
    path: &str,
    offset: usize,
) -> Vec<ParserExpectedSyntax> {
    slang_adapter::expected_syntax_at_offset(text, name, path, offset)
}

pub fn expected_syntax_at_offset_with_options(
    text: &str,
    name: &str,
    path: &str,
    offset: usize,
    options: &SyntaxTreeOptions,
) -> Vec<ParserExpectedSyntax> {
    slang_adapter::expected_syntax_at_offset_with_options(text, name, path, offset, options)
}

pub fn library_map_expected_syntax_at_offset(
    text: &str,
    name: &str,
    path: &str,
    offset: usize,
) -> Vec<ParserExpectedSyntax> {
    slang_adapter::library_map_expected_syntax_at_offset(text, name, path, offset)
}

pub fn directive_at_offset(
    text: &str,
    name: &str,
    path: &str,
    offset: usize,
) -> Option<LexedTokenAtOffset> {
    slang_adapter::directive_at_offset(text, name, path, offset)
}

pub fn token_word_at_offset(
    text: &str,
    name: &str,
    path: &str,
    offset: usize,
) -> Option<LexedTokenAtOffset> {
    slang_adapter::token_word_at_offset(text, name, path, offset)
}

pub fn preproc_file_index_from_text(text: &str, options: &SyntaxTreeOptions) -> PreprocFileIndex {
    slang_adapter::preproc_file_index_from_text(text, options)
}

pub fn literal_include_directives(text: &str) -> Vec<MacroInclude> {
    slang_adapter::literal_include_directives(text)
}

pub fn system_function_names() -> Vec<String> {
    slang_adapter::system_function_names()
}

pub fn system_task_names() -> Vec<String> {
    slang_adapter::system_task_names()
}

pub struct Compilation {
    inner: slang_adapter::Compilation,
}

impl Default for Compilation {
    fn default() -> Self {
        Self::new()
    }
}

impl Compilation {
    pub fn new() -> Self {
        Self { inner: slang_adapter::Compilation::new() }
    }

    pub fn new_with_top_modules(top_modules: &[String]) -> Self {
        Self { inner: slang_adapter::Compilation::new_with_top_modules(top_modules) }
    }

    pub fn add_syntax_tree_from_text(
        &mut self,
        text: &str,
        name: &str,
        path: &str,
        options: &SyntaxTreeOptions,
    ) -> SyntaxTreeBufferIds {
        self.inner.add_syntax_tree_from_text(text, name, path, options)
    }

    pub fn add_library_map_syntax_tree_from_text(
        &mut self,
        text: &str,
        name: &str,
        path: &str,
    ) -> SyntaxTreeBufferIds {
        self.inner.add_library_map_syntax_tree_from_text(text, name, path)
    }

    pub fn parse_diagnostics_with_options(
        &self,
        warning_options: &[String],
    ) -> Vec<SyntaxDiagnostic> {
        self.inner.parse_diagnostics_with_options(warning_options)
    }

    pub fn semantic_diagnostics_with_options(
        &self,
        warning_options: &[String],
    ) -> Vec<SyntaxDiagnostic> {
        self.inner.semantic_diagnostics_with_options(warning_options)
    }
}
