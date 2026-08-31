mod ffi;

use std::pin::Pin;

use cxx::{SharedPtr, UniquePtr};

use crate::{
    diagnostic::SyntaxDiagnostic,
    syntax::{SyntaxTree, SyntaxTreeBuffer, SyntaxTreeOptions, ffi as syntax_ffi},
};

/// A `SourceManager` that outlives the compilations built on it.
///
/// Parse trees belong to the session. A [`Compilation`] is a value: add trees,
/// query, drop. Editing a file is [`Self::replace_buffer`], not a new session.
pub struct SourceSession {
    raw: SharedPtr<syntax_ffi::SourceSession>,
}

impl SourceSession {
    pub fn new() -> Self {
        Self { raw: syntax_ffi::new_source_session() }
    }

    pub fn assign(&self, path: &str, text: &str) {
        syntax_ffi::source_session_assign_text(self.raw.clone(), path, text);
    }

    /// Put new text at an already-assigned path.
    ///
    /// This is the edit step, not a new session and not a new compilation.
    /// Trees parsed before the replace stay valid. The next parse of this
    /// path uses the new text.
    pub fn replace_buffer(&self, path: &str, text: &str) {
        syntax_ffi::source_session_replace_buffer(self.raw.clone(), path, text);
    }

    pub fn parse(&self, name: &str, path: &str, options: &SyntaxTreeOptions) -> SyntaxTree {
        SyntaxTree::from_raw(syntax_ffi::source_session_parse(
            self.raw.clone(),
            name,
            path,
            options.predefines.clone(),
            options.include_paths.clone(),
            options.expand_includes,
            options.collect_expected_syntax,
            options.expected_syntax_offset.unwrap_or_default(),
            options.expected_syntax_offset.is_some(),
        ))
    }

    pub fn parse_text(
        &self,
        text: &str,
        name: &str,
        path: &str,
        options: &SyntaxTreeOptions,
    ) -> SyntaxTree {
        SyntaxTree::from_raw(syntax_ffi::source_session_parse_text(
            self.raw.clone(),
            text,
            name,
            path,
            options.predefines.clone(),
            options.include_paths.clone(),
            options.expand_includes,
            options.collect_expected_syntax,
            options.expected_syntax_offset.unwrap_or_default(),
            options.expected_syntax_offset.is_some(),
        ))
    }

    pub fn parse_library_map(
        &self,
        name: &str,
        path: &str,
        options: &SyntaxTreeOptions,
    ) -> SyntaxTree {
        SyntaxTree::from_raw(syntax_ffi::source_session_parse_library_map(
            self.raw.clone(),
            name,
            path,
            options.collect_expected_syntax,
            options.expected_syntax_offset.unwrap_or_default(),
            options.expected_syntax_offset.is_some(),
        ))
    }

    pub fn parse_count(&self) -> u32 {
        syntax_ffi::source_session_parse_count(self.raw.clone())
    }
}

impl Default for SourceSession {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for SourceSession {
    fn clone(&self) -> Self {
        Self { raw: self.raw.clone() }
    }
}

pub struct Compilation {
    raw: UniquePtr<ffi::Compilation>,
}

/// One elaborated instance: hierarchical path and the instantiation site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierInstance {
    pub path: String,
    pub file: String,
    pub offset: usize,
}

/// Symbol at a source offset: type and definition site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolInfo {
    pub name: String,
    pub type_name: String,
    pub kind: String,
    pub def_file: String,
    pub def_offset: usize,
    pub owner_class: String,
    pub inheritance: Vec<String>,
}

/// A member of a scope or structured type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberInfo {
    pub name: String,
    pub type_name: String,
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

    /// A compilation that uses `session`'s source manager. Dropping it does
    /// not drop the session or the trees parsed on it.
    pub fn on(session: &SourceSession) -> Self {
        Self::on_with_top_modules(session, &[])
    }

    pub fn on_with_top_modules(session: &SourceSession, top_modules: &[String]) -> Self {
        Self { raw: ffi::new_compilation_on_session(session.raw.clone(), top_modules.to_vec()) }
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

    /// Semantic answer for the symbol at `offset` in `path`.
    ///
    /// `None` means slang elaborated the compilation and the offset denotes
    /// no symbol. For a class member the answer also carries the owning
    /// class and its base-class chain.
    pub fn lookup_symbol(&mut self, path: &str, offset: usize) -> Option<SymbolInfo> {
        let answer =
            ffi::lookup_symbol(self.raw_pin(), path, offset).unwrap_or_else(|err| panic!("{err}"));
        answer.found.then_some(SymbolInfo {
            name: answer.name,
            type_name: answer.type_name,
            kind: answer.kind,
            def_file: answer.def_file,
            def_offset: answer.def_offset,
            owner_class: answer.owner_class,
            inheritance: answer.inheritance,
        })
    }

    pub fn lookup_scoped(&mut self, left: &str, right: &str) -> Option<SymbolInfo> {
        let answer = ffi::lookup_scoped(self.raw_pin(), left, right);
        answer.found.then_some(SymbolInfo {
            name: answer.name,
            type_name: answer.type_name,
            kind: answer.kind,
            def_file: answer.def_file,
            def_offset: answer.def_offset,
            owner_class: answer.owner_class,
            inheritance: answer.inheritance,
        })
    }

    pub fn list_members(&mut self, path: &str, offset: usize) -> Vec<MemberInfo> {
        ffi::list_members(self.raw_pin(), path, offset)
            .unwrap_or_else(|err| panic!("{err}"))
            .into_iter()
            .map(|row| MemberInfo { name: row.name, type_name: row.type_name })
            .collect()
    }

    pub fn list_scope_members(&mut self, name: &str) -> Vec<MemberInfo> {
        ffi::list_scope_members(self.raw_pin(), name)
            .into_iter()
            .map(|row| MemberInfo { name: row.name, type_name: row.type_name })
            .collect()
    }

    pub fn lookup_type(&mut self, path: &str, start: usize, end: usize) -> Option<String> {
        let answer = ffi::lookup_type(self.raw_pin(), path, start, end)
            .unwrap_or_else(|err| panic!("{err}"));
        answer.found.then_some(answer.type_name)
    }

    pub fn list_instances(&mut self) -> Vec<HierInstance> {
        ffi::list_instances(self.raw_pin())
            .into_iter()
            .map(|row| HierInstance { path: row.path, file: row.file, offset: row.offset })
            .collect()
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
            .lookup_symbol("uvm_object.svh", offset)
            .expect("slang must see the UVM-shaped class property");
        assert_eq!(info.owner_class, "uvm_object");
        assert!(info.inheritance.iter().any(|name| name == "uvm_void"), "{info:?}");
        assert!(info.type_name.contains("string"), "{info:?}");
    }

    #[test]
    fn lookup_uses_the_assigned_buffer_path() {
        let src = "virtual class uvm_void; endclass\nvirtual class uvm_object extends uvm_void;\n  string m_leaf_name;\nendclass\n";
        let path = "/vide-assigned/uvm_object.svh";
        let mut compilation = Compilation::new();
        compilation.parse_syntax_tree_from_text(
            src,
            "uvm_object.svh",
            path,
            &SyntaxTreeOptions::default(),
        );
        let offset = src.find("m_leaf_name").expect("property");
        let info = compilation
            .lookup_symbol(path, offset)
            .expect("lookup must hit the buffer under the path it was assigned");
        assert_eq!(info.owner_class, "uvm_object");
        assert!(info.type_name.contains("string"), "{info:?}");
    }

    #[test]
    fn list_instances_reports_hierarchical_path_and_site() {
        let src = "module child; endmodule\nmodule top; child u0(); endmodule\n";
        let mut compilation = Compilation::new();
        compilation.parse_syntax_tree_from_text(
            src,
            "top",
            "top.sv",
            &SyntaxTreeOptions::default(),
        );
        let instances = compilation.list_instances();
        assert!(
            instances.iter().any(|inst| inst.path.contains("u0") && inst.file == "top.sv"),
            "{instances:?}"
        );
    }

    #[test]
    fn list_instances_reports_the_assigned_buffer_path() {
        let src = "module child; endmodule\nmodule top; child u0(); endmodule\n";
        let path = "/vide-assigned/top.sv";
        let mut compilation = Compilation::new();
        compilation.parse_syntax_tree_from_text(src, "top", path, &SyntaxTreeOptions::default());
        let instances = compilation.list_instances();
        let inst = instances
            .iter()
            .find(|inst| inst.path.contains("u0"))
            .unwrap_or_else(|| panic!("missing u0: {instances:?}"));
        assert_eq!(inst.file, path, "{instances:?}");
    }

    #[test]
    fn lookup_symbol_answers_a_net_type_and_a_class_scope() {
        let src = r#"
class env;
  static int count;
endclass
module top;
  logic [7:0] x;
  initial env::count = x;
endmodule
"#;
        let path = "/vide-assigned/top.sv";
        let mut compilation = Compilation::new();
        compilation.parse_syntax_tree_from_text(src, "top", path, &SyntaxTreeOptions::default());
        let x = compilation
            .lookup_symbol(path, src.find("x;").expect("net"))
            .expect("net at its declaration");
        assert!(x.type_name.contains("logic"), "{x:?}");
        let scoped = compilation
            .lookup_symbol(path, src.find("count =").expect("class scope"))
            .expect("env::count at the use");
        assert_eq!(scoped.name, "count", "{scoped:?}");
        assert!(scoped.type_name.contains("int"), "{scoped:?}");
        assert_eq!(scoped.def_file, path, "{scoped:?}");
        assert_eq!(scoped.def_offset, src.find("count;").expect("def"), "{scoped:?}");
    }

    #[test]
    fn lookup_scoped_resolves_package_and_class() {
        let src = r#"
package p;
  typedef logic exported_t;
endpackage
class env;
  static int count;
endclass
module top;
  p::exported_t x;
  initial env::count = 1;
endmodule
"#;
        let mut compilation = Compilation::new();
        compilation.parse_syntax_tree_from_text(
            src,
            "top",
            "/vide-assigned/top.sv",
            &SyntaxTreeOptions::default(),
        );
        let exported = compilation.lookup_scoped("p", "exported_t").expect("p::exported_t");
        assert_eq!(exported.name, "exported_t", "{exported:?}");
        let count = compilation.lookup_scoped("env", "count").expect("env::count");
        assert_eq!(count.name, "count", "{count:?}");
        let pkg = compilation.lookup_scoped("p", "").expect("package p");
        assert_eq!(pkg.name, "p", "{pkg:?}");
    }

    #[test]
    fn list_members_of_a_package_and_a_struct() {
        let src = r#"
package p;
  typedef logic exported_t;
  function int make(); return 1; endfunction
endpackage
module top;
  typedef struct { logic [7:0] field; } packet_t;
  packet_t pkt;
endmodule
"#;
        let path = "/vide-assigned/members.sv";
        let mut compilation = Compilation::new();
        compilation.parse_syntax_tree_from_text(src, "top", path, &SyntaxTreeOptions::default());
        let pkg_members = compilation.list_members(path, src.find("p;").expect("package name"));
        let names: Vec<_> = pkg_members.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"exported_t"), "{pkg_members:?}");
        assert!(names.contains(&"make"), "{pkg_members:?}");
        let fields = compilation.list_members(path, src.find("pkt;").expect("pkt"));
        assert!(fields.iter().any(|m| m.name == "field"), "{fields:?}");
    }

    #[test]
    fn list_scope_members_of_hierarchical_instance_and_struct() {
        let src = r#"
package p;
  typedef logic exported_t;
  function int make(); return 1; endfunction
endpackage
module leaf;
  wire leaf_wire;
endmodule
module top;
  leaf u0();
  typedef struct { logic [7:0] field; } packet_t;
  packet_t pkt;
endmodule
"#;
        let path = "/vide-assigned/hier.sv";
        let mut compilation = Compilation::new();
        compilation.parse_syntax_tree_from_text(src, "top", path, &SyntaxTreeOptions::default());
        let inst = compilation.list_scope_members("top.u0");
        assert!(inst.iter().any(|m| m.name == "leaf_wire"), "top.u0 members: {inst:?}");
        let nested = compilation.list_scope_members("u0");
        assert!(nested.iter().any(|m| m.name == "leaf_wire"), "u0 members: {nested:?}");
        let fields = compilation.list_scope_members("pkt");
        assert!(fields.iter().any(|m| m.name == "field"), "pkt members: {fields:?}");
        let pkg = compilation.list_scope_members("p");
        let pkg_names: Vec<_> = pkg.iter().map(|m| m.name.as_str()).collect();
        assert!(pkg_names.contains(&"exported_t"), "{pkg:?}");
        assert!(pkg_names.contains(&"make"), "{pkg:?}");
    }

    #[test]
    fn lookup_type_covers_an_additive_expression() {
        let src = "module top; logic [7:0] a, b, y; always_comb y = a + b; endmodule\n";
        let path = "/vide-assigned/add.sv";
        let mut compilation = Compilation::new();
        compilation.parse_syntax_tree_from_text(src, "top", path, &SyntaxTreeOptions::default());
        let start = src.find("a + b").expect("expr");
        let end = start + "a + b".len();
        let ty = compilation.lookup_type(path, start, end).expect("type of a + b");
        assert!(ty.contains("logic"), "{ty}");
    }

    #[test]
    fn list_scope_members_of_a_select_prefix_does_not_abort() {
        let src = "module top; logic [7:0] bus; initial bus[0] = 1; endmodule\n";
        let path = "/vide-assigned/select.sv";
        let mut compilation = Compilation::new();
        compilation.parse_syntax_tree_from_text(src, "top", path, &SyntaxTreeOptions::default());
        let members = compilation.list_scope_members("bus[0]");
        assert!(members.is_empty(), "element select is not a hierarchical name: {members:?}");
    }

    #[test]
    fn lookup_type_of_mixed_width_add_is_the_sum_not_the_narrow_operand() {
        let src = "module top; logic [3:0] b; logic [7:0] a, y; always_comb y = b + a; endmodule\n";
        let path = "/vide-assigned/add-mixed.sv";
        let mut compilation = Compilation::new();
        compilation.parse_syntax_tree_from_text(src, "top", path, &SyntaxTreeOptions::default());
        let start = src.find("b + a").expect("expr");
        let end = start + "b + a".len();
        let ty = compilation.lookup_type(path, start, end).expect("type of b + a");
        assert!(
            ty.contains("logic") && ty.contains("7"),
            "sum of logic[3:0] + logic[7:0] must be the 8-bit result, not operand b: {ty}"
        );
        assert!(!ty.contains("[3:0]"), "must not return the narrow operand type: {ty}");
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

    #[test]
    fn session_outlives_compilation() {
        let session = SourceSession::new();
        let tree =
            session.parse_text("module m; endmodule\n", "m", "m.sv", &SyntaxTreeOptions::default());
        let mut first = Compilation::on(&session);
        first.add_syntax_tree(&tree);
        drop(first);
        let mut second = Compilation::on(&session);
        second.add_syntax_tree(&tree);
        assert!(second.parse_diagnostics_with_options(&[]).is_empty());
    }

    #[test]
    fn replace_buffer_leaves_the_old_tree_and_parses_the_new_text() {
        let session = SourceSession::new();
        session.assign("a.sv", "module a; endmodule\n");
        session.assign("b.sv", "module b; endmodule\n");
        let tree_a = session.parse("a", "a.sv", &SyntaxTreeOptions::default());
        let tree_b = session.parse("b", "b.sv", &SyntaxTreeOptions::default());
        let edited = "module a; logic x; endmodule\n";
        session.replace_buffer("a.sv", edited);
        assert_eq!(
            tree_a.root().kind(),
            SyntaxKind::COMPILATION_UNIT,
            "trees parsed before replace keep working"
        );
        assert_eq!(tree_b.root().kind(), SyntaxKind::COMPILATION_UNIT);

        let new_a = session.parse("a", "a.sv", &SyntaxTreeOptions::default());
        let mut compilation = Compilation::on(&session);
        compilation.add_syntax_tree(&new_a);
        compilation.add_syntax_tree(&tree_b);
        let info = compilation
            .lookup_symbol("a.sv", edited.find("x;").expect("new net"))
            .expect("new parse must see the replacement text");
        assert!(info.type_name.contains("logic"), "{info:?}");
    }

    #[test]
    fn a_session_tree_is_reused_on_a_new_compilation() {
        let pkg = "package pkg;\n  class leaf;\n    string m_leaf_name;\n  endclass\nendpackage\n";
        let user = "module top;\n  import pkg::*;\n  leaf inst;\n  initial inst.m_leaf_name = \"x\";\nendmodule\n";
        let user_edit = "module top;\n  import pkg::*;\n  leaf inst;\n  initial inst.m_leaf_name = \"y\";\nendmodule\n";
        let session = SourceSession::new();
        let pkg_tree = session.parse_text(pkg, "pkg", "pkg.sv", &SyntaxTreeOptions::default());
        let user_tree = session.parse_text(user, "user", "user.sv", &SyntaxTreeOptions::default());
        assert_eq!(session.parse_count(), 2);

        let mut first = Compilation::on(&session);
        first.add_syntax_tree(&pkg_tree);
        first.add_syntax_tree(&user_tree);
        let offset = user.find("m_leaf_name").expect("use");
        let info = first
            .lookup_symbol("user.sv", offset)
            .expect("C1 must see the class member through the import");
        assert!(info.type_name.contains("string"), "{info:?}");
        assert_eq!(info.owner_class, "leaf", "{info:?}");
        assert!(first.parse_diagnostics_with_options(&[]).is_empty());
        let ty = first
            .lookup_type("user.sv", offset, offset + "m_leaf_name".len())
            .expect("C1 type lookup must work on the same trees");
        assert!(ty.contains("string"), "{ty}");
        drop(first);

        session.replace_buffer("user.sv", user_edit);
        let user_tree = session.parse("user", "user.sv", &SyntaxTreeOptions::default());
        assert_eq!(session.parse_count(), 3, "pkg.sv must not be parsed a second time");

        let mut second = Compilation::on(&session);
        second.add_syntax_tree(&pkg_tree);
        second.add_syntax_tree(&user_tree);
        let info = second
            .lookup_symbol("user.sv", user_edit.find("m_leaf_name").expect("use"))
            .expect("C2 must see the member after reusing the pkg tree");
        assert!(info.type_name.contains("string"), "{info:?}");
        let ty = second
            .lookup_type(
                "user.sv",
                user_edit.find("m_leaf_name").expect("use"),
                user_edit.find("m_leaf_name").expect("use") + "m_leaf_name".len(),
            )
            .expect("C2 type lookup must work after reusing the pkg tree");
        assert!(ty.contains("string"), "{ty}");
        assert!(second.parse_diagnostics_with_options(&[]).is_empty());
        let semantic = second.semantic_diagnostics_with_options(&[]);
        assert!(
            semantic.iter().all(|d| d.severity != crate::diagnostic::DiagnosticSeverity::Error),
            "{semantic:?}"
        );
    }

    /// After replace, a live earlier Compilation must keep answering from the
    /// trees it was given. The session's latest buffer is not this
    /// compilation's source.
    #[test]
    fn replaced_path_does_not_steal_an_earlier_compilation_lookup() {
        let pkg = "package pkg;\n  class leaf;\n    string m_leaf_name;\n  endclass\nendpackage\n";
        let user = "module top;\n  import pkg::*;\n  leaf inst;\n  initial inst.m_leaf_name = \"x\";\nendmodule\n";
        let user_edit = "module top;\n  import pkg::*;\n  leaf inst;\n  logic extra_signal;\n  initial inst.m_leaf_name = \"y\";\nendmodule\n";
        let session = SourceSession::new();
        let pkg_tree = session.parse_text(pkg, "pkg", "pkg.sv", &SyntaxTreeOptions::default());
        let user_tree = session.parse_text(user, "user", "user.sv", &SyntaxTreeOptions::default());

        let mut first = Compilation::on(&session);
        first.add_syntax_tree(&pkg_tree);
        first.add_syntax_tree(&user_tree);
        let old_member = first
            .lookup_symbol("user.sv", user.find("m_leaf_name").expect("use"))
            .expect("C1 sees the class member before the edit");
        assert_eq!(old_member.owner_class, "leaf", "{old_member:?}");

        session.replace_buffer("user.sv", user_edit);
        let new_user = session.parse("user", "user.sv", &SyntaxTreeOptions::default());
        let mut second = Compilation::on(&session);
        second.add_syntax_tree(&pkg_tree);
        second.add_syntax_tree(&new_user);

        assert_eq!(
            user_tree.root().kind(),
            SyntaxKind::COMPILATION_UNIT,
            "old user tree must remain valid after replace"
        );
        let extra_offset = user_edit.find("extra_signal").expect("new-only name");
        let extra = second
            .lookup_symbol("user.sv", extra_offset)
            .expect("C2 must see the name that exists only in the new buffer");
        assert_eq!(extra.name, "extra_signal", "{extra:?}");

        let stolen = first.lookup_symbol("user.sv", extra_offset);
        assert!(
            stolen.is_none(),
            "C1 must keep the old user buffer; the session's latest buffer is C2's: {stolen:?}"
        );
        let still = first
            .lookup_symbol("user.sv", user.find("m_leaf_name").expect("use"))
            .expect("C1 must still answer from the old tree");
        assert_eq!(still.owner_class, "leaf", "{still:?}");
    }

    fn panic_text(payload: Box<dyn std::any::Any + Send>) -> String {
        payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|text| (*text).to_owned()))
            .unwrap_or_else(|| "non-string panic".to_owned())
    }

    fn mapping_failure<T: std::fmt::Debug>(result: std::thread::Result<T>, quiet: &str) {
        match result {
            Ok(value) => panic!("{quiet}: {value:?}"),
            Err(payload) => {
                let text = panic_text(payload);
                assert!(
                    text.contains("not in this compilation"),
                    "mapping failure must name the missing path, got: {text}"
                );
            }
        }
    }

    #[test]
    fn two_compilations_on_one_session_can_add_the_same_trees() {
        let session = SourceSession::new();
        let src = "module m; logic x; endmodule\n";
        let tree = session.parse_text(src, "m", "m.sv", &SyntaxTreeOptions::default());
        let mut first = Compilation::on(&session);
        first.add_syntax_tree(&tree);
        let mut second = Compilation::on(&session);
        second.add_syntax_tree(&tree);
        let offset = src.find("x;").expect("net");
        let first_info = first.lookup_symbol("m.sv", offset).expect("C1 lookup");
        let second_info = second.lookup_symbol("m.sv", offset).expect("C2 lookup");
        assert!(first_info.type_name.contains("logic"), "{first_info:?}");
        assert!(second_info.type_name.contains("logic"), "{second_info:?}");
        assert!(first.parse_diagnostics_with_options(&[]).is_empty());
        assert!(second.parse_diagnostics_with_options(&[]).is_empty());
    }

    #[test]
    fn path_mapping_failure_is_not_a_silent_empty_result() {
        let mut compilation = Compilation::new();
        compilation.parse_syntax_tree_from_text(
            "module top; logic x; endmodule\n",
            "top",
            "top.sv",
            &SyntaxTreeOptions::default(),
        );

        mapping_failure(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                compilation.lookup_symbol("other.sv", 0)
            })),
            "lookup_symbol of a path this compilation does not own must not look like a miss",
        );
        mapping_failure(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                compilation.list_members("other.sv", 0)
            })),
            "list_members of a path this compilation does not own must not return an empty scope",
        );
        mapping_failure(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                compilation.lookup_type("other.sv", 0, 1)
            })),
            "lookup_type of a path this compilation does not own must not look like a miss",
        );
    }
}
