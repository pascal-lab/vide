mod ffi;

use std::pin::Pin;

use cxx::UniquePtr;

use crate::{
    diagnostic::SyntaxDiagnostic,
    syntax::{SyntaxTree, SyntaxTreeBuffer, SyntaxTreeOptions},
};

pub struct Compilation {
    raw: UniquePtr<ffi::Compilation>,
}

/// Type, owning class, and base-class chain of one class member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassMemberInfo {
    pub type_name: String,
    pub owner_class: String,
    pub inheritance: Vec<String>,
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

    pub fn add_syntax_tree(&mut self, tree: &SyntaxTree) {
        ffi::add_syntax_tree(self.raw_pin(), tree.raw.clone());
    }

    pub fn register_source_buffers(&mut self, buffers: &[SyntaxTreeBuffer]) {
        ffi::register_source_buffers(
            self.raw_pin(),
            buffers.iter().map(|buffer| buffer.path.clone()).collect(),
            buffers.iter().map(|buffer| buffer.text.clone()).collect(),
        );
    }

    pub fn parse_syntax_tree_from_text(
        &mut self,
        text: &str,
        name: &str,
        path: &str,
        options: &SyntaxTreeOptions,
    ) -> SyntaxTree {
        let raw_options = self.raw_parse_options(options);
        SyntaxTree::from_raw(ffi::parse_syntax_tree_from_text(
            self.raw_pin(),
            text,
            name,
            path,
            raw_options,
        ))
    }

    pub fn parse_syntax_tree_from_buffer(
        &mut self,
        name: &str,
        path: &str,
        options: &SyntaxTreeOptions,
    ) -> SyntaxTree {
        if !options.include_buffers.is_empty() {
            panic!("buffer parsing requires source buffers to be registered on the compilation");
        }
        let raw_options = self.raw_parse_options(options);
        SyntaxTree::from_raw(ffi::parse_syntax_tree_from_buffer(
            self.raw_pin(),
            name,
            path,
            raw_options,
        ))
    }

    pub fn parse_library_map_syntax_tree_from_text(
        &mut self,
        text: &str,
        name: &str,
        path: &str,
    ) -> SyntaxTree {
        SyntaxTree::from_raw(ffi::parse_library_map_syntax_tree_from_text(
            self.raw_pin(),
            text,
            name,
            path,
        ))
    }

    pub fn parse_library_map_syntax_tree_from_buffer(
        &mut self,
        name: &str,
        path: &str,
        options: &SyntaxTreeOptions,
    ) -> SyntaxTree {
        if !options.include_buffers.is_empty()
            || !options.predefines.is_empty()
            || !options.include_paths.is_empty()
            || !options.expand_includes
        {
            panic!("library map buffer parsing received unsupported syntax options");
        }
        SyntaxTree::from_raw(ffi::parse_library_map_syntax_tree_from_buffer(
            self.raw_pin(),
            name,
            path,
            options.collect_expected_syntax,
            options.expected_syntax_offset.unwrap_or_default(),
            options.expected_syntax_offset.is_some(),
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

    /// Semantic answer for a class member at `offset` in `path`.
    ///
    /// Empty `found` means slang elaborated the compilation but the offset
    /// is not a class property or subroutine. This is the T4 slice: type,
    /// owning class, inheritance chain.
    pub fn lookup_class_member(&mut self, path: &str, offset: usize) -> Option<ClassMemberInfo> {
        let answer = ffi::lookup_class_member(self.raw_pin(), path, offset);
        answer.found.then_some(ClassMemberInfo {
            type_name: answer.type_name,
            owner_class: answer.owner_class,
            inheritance: answer.inheritance,
        })
    }

    fn raw_pin(&mut self) -> Pin<&mut ffi::Compilation> {
        self.raw.as_mut().expect("Slang compilation unexpectedly null")
    }

    fn raw_parse_options(&self, options: &SyntaxTreeOptions) -> ffi::ParseSyntaxTreeOptions {
        ffi::ParseSyntaxTreeOptions {
            predefines: options.predefines.clone(),
            include_paths: options.include_paths.clone(),
            include_buffer_paths: options
                .include_buffers
                .iter()
                .map(|buffer| buffer.path.clone())
                .collect(),
            include_buffer_texts: options
                .include_buffers
                .iter()
                .map(|buffer| buffer.text.clone())
                .collect(),
            expand_includes: options.expand_includes,
            collect_expected_syntax: options.collect_expected_syntax,
            expected_syntax_offset: options.expected_syntax_offset.unwrap_or_default(),
            has_expected_syntax_offset: options.expected_syntax_offset.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{SyntaxKind, SyntaxTreeBuffer};

    #[test]
    fn adding_a_cloned_tree_does_not_invalidate_the_original() {
        let mut parser = Compilation::new();
        let tree = parser.parse_syntax_tree_from_text(
            "module demo; endmodule",
            "source",
            "source.sv",
            &SyntaxTreeOptions::default(),
        );
        let original = tree.clone();

        let mut compilation = Compilation::new();
        compilation.add_syntax_tree(&tree);

        assert_eq!(original.root().kind(), SyntaxKind::COMPILATION_UNIT);
    }

    #[test]
    fn compilation_keeps_an_attached_tree_source_session_alive() {
        let tree =
            SyntaxTree::from_file_in_memory("module demo; endmodule\n", "source", "source.sv");
        let mut compilation = Compilation::new();
        compilation.add_syntax_tree(&tree);
        drop(tree);

        assert!(compilation.parse_diagnostics_with_options(&[]).is_empty());
    }

    #[test]
    fn uvm_shaped_class_member_has_type_class_and_inheritance() {
        let src = r#"
virtual class uvm_void;
endclass
virtual class uvm_object extends uvm_void;
  string m_leaf_name;
  function string get_type_name();
    return "";
  endfunction
endclass
"#;
        let mut compilation = Compilation::new();
        compilation.parse_syntax_tree_from_text(
            src,
            "uvm_object.svh",
            "uvm_object.svh",
            &SyntaxTreeOptions::default(),
        );
        let offset = src.find("m_leaf_name").expect("property");
        let info = compilation
            .lookup_class_member("uvm_object.svh", offset)
            .expect("slang must see the UVM-shaped class property");
        assert_eq!(info.owner_class, "uvm_object");
        assert!(info.inheritance.iter().any(|name| name == "uvm_void"), "{info:?}");
        assert!(info.type_name.contains("string"), "{info:?}");
    }

    #[test]
    fn empty_compilation_has_no_diagnostics() {
        let compilation = Compilation::new();

        assert!(compilation.parse_diagnostics_with_options(&[]).is_empty());
        assert!(compilation.semantic_diagnostics_with_options(&[]).is_empty());
    }

    #[test]
    fn registered_source_buffers_are_available_before_root_parsing() {
        let mut compilation = Compilation::new();
        compilation.register_source_buffers(&[
            SyntaxTreeBuffer {
                path: "root.sv".to_owned(),
                text: "`define HEADER \"header.svh\"\n`include `HEADER\nmodule root; endmodule\n"
                    .to_owned(),
            },
            SyntaxTreeBuffer {
                path: "header.svh".to_owned(),
                text: "`define ROOT_VALUE 1\n".to_owned(),
            },
        ]);

        let tree = compilation.parse_syntax_tree_from_buffer(
            "root",
            "root.sv",
            &SyntaxTreeOptions::default(),
        );

        assert_eq!(tree.root().kind(), SyntaxKind::COMPILATION_UNIT);
        assert!(compilation.parse_diagnostics_with_options(&[]).is_empty());
    }
}
