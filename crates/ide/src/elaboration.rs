//! Profile extras that used to live on a dedicated `vide-elaboration` thread.
//!
//! That worker is gone. Instances compile on the calling thread through
//! [`crate::compile::Compiler`], the same function hover uses.

#[cfg(test)]
mod tests {
    use base_db::{
        change::Change,
        project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
        source_root::{SourceRoot, SourceRootId},
    };
    use triomphe::Arc;
    use utils::{line_index::TextSize, paths::AbsPathBuf};
    use vfs::{ChangedFile, FileId, FileSet, VfsPath};

    use crate::{
        analysis_host::AnalysisHost,
        slang_class,
        test_utils::{setup_marked, setup_with_path},
    };

    const OBJECT: &str = r#"
virtual class uvm_void;
endclass
virtual class uvm_object extends uvm_void;
  string /*marker:name*/m_leaf_name;
endclass
"#;

    fn lookup_at(
        host: &AnalysisHost,
        file_id: FileId,
        offset: TextSize,
    ) -> Option<slang_sys::compilation::SymbolInfo> {
        slang_class::lookup_symbol_at(&host.ctx(), file_id, usize::from(offset))
    }

    #[test]
    fn analysis_host_new_answers_hover_without_an_elaboration_thread() {
        let (host, file_id, _text, markers) = setup_marked(OBJECT);
        let hover = host
            .make_analysis()
            .hover(crate::FilePosition { file_id, offset: markers["name"] })
            .unwrap()
            .expect("hover");
        assert!(
            hover.info.as_str().contains("string"),
            "production AnalysisHost::new must hover without vide-elaboration: {}",
            hover.info.as_str()
        );
    }

    #[test]
    fn class_member_lookup_is_some_and_a_miss_is_none() {
        let (host, file_id, _text, markers) = setup_marked(OBJECT);
        let info = lookup_at(&host, file_id, markers["name"]).expect("class property");
        assert_eq!(info.owner_class, "uvm_object");
        assert!(info.inheritance.iter().any(|name| name == "uvm_void"), "{info:?}");
        assert!(info.type_name.contains("string"), "{info:?}");
        assert!(lookup_at(&host, file_id, TextSize::from(0u32)).is_none());
    }

    #[test]
    fn a_real_file_set_resolves_cross_file_inheritance() {
        let root = AbsPathBuf::assert(
            if cfg!(windows) { "C:/vide-elab-cross" } else { "/vide-elab-cross" }.into(),
        );
        let pkg_path = root.join("uvm_pkg.sv");
        let user_path = root.join("user.sv");
        let mut file_set = FileSet::default();
        file_set.insert(FileId::from_raw(0), VfsPath::from(pkg_path));
        file_set.insert(FileId::from_raw(1), VfsPath::from(user_path));

        let mut change = Change::new();
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.set_project_config(Arc::new(ProjectConfig::new(
            vec![Some(CompilationProfileId(0))],
            vec![CompilationProfile {
                source_roots: vec![SourceRootId(0)],
                top_modules: Vec::new(),
                preprocess: PreprocessConfig {
                    include_dirs: vec![root],
                    ..PreprocessConfig::default()
                },
            }],
        )));
        change.add_changed_file(ChangedFile::create(
            FileId::from_raw(0),
            "package uvm_pkg;\n  virtual class uvm_void;\n  endclass\n  virtual class uvm_object extends uvm_void;\n  endclass\nendpackage\n",
        ));
        let user = "package p;\n  import uvm_pkg::*;\n  class child extends uvm_object;\n    string m_leaf_name;\n  endclass\nendpackage\n";
        change.add_changed_file(ChangedFile::create(FileId::from_raw(1), user));

        let mut host = AnalysisHost::new(None);
        host.apply_change(change);
        let offset = TextSize::from(user.find("m_leaf_name").unwrap() as u32);
        let info = lookup_at(&host, FileId::from_raw(1), offset).expect("cross-file class member");
        assert_eq!(info.owner_class, "child");
        assert!(
            info.inheritance.iter().any(|name| name == "uvm_object" || name == "uvm_void"),
            "inheritance must resolve through the imported package in the same compilation: {info:?}"
        );
    }

    #[test]
    fn instance_hierarchy_names_the_instantiation_site() {
        let src = "module child; endmodule\nmodule top; child u0(); endmodule\n";
        let (host, file_id) = setup_with_path(src, "/top.sv");
        let ctx = host.ctx();
        let mut artifact = ctx.profile_compilation(ctx.db.file_compilation_profile(file_id));
        let rows = artifact.list_instances();
        let u0 =
            rows.iter().find(|row| row.path.contains("u0")).unwrap_or_else(|| panic!("{rows:?}"));
        let site = src.find("u0").expect("instance name");
        assert_eq!(u0.offset, site, "{u0:?}");
        assert!(
            rows.iter().any(|row| row.offset == site && row.path.contains("u0")),
            "source site must list the instance: {rows:?}"
        );
    }

    #[test]
    fn an_edit_keeps_the_other_roots_answerable() {
        let root = AbsPathBuf::assert(
            if cfg!(windows) { "C:/vide-elab-reuse" } else { "/vide-elab-reuse" }.into(),
        );
        let class_file = FileId::from_raw(0);
        let module_file = FileId::from_raw(1);
        let mut file_set = FileSet::default();
        file_set.insert(class_file, VfsPath::from(root.join("a.sv")));
        file_set.insert(module_file, VfsPath::from(root.join("b.sv")));

        let class_text = "class holder;\n  string tag;\nendclass\n";
        let mut change = Change::new();
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.set_project_config(Arc::new(ProjectConfig::new(
            vec![Some(CompilationProfileId(0))],
            vec![CompilationProfile {
                source_roots: vec![SourceRootId(0)],
                top_modules: Vec::new(),
                preprocess: PreprocessConfig {
                    include_dirs: vec![root],
                    ..PreprocessConfig::default()
                },
            }],
        )));
        change.add_changed_file(ChangedFile::create(class_file, class_text));
        change.add_changed_file(ChangedFile::create(module_file, "module b;\nendmodule\n"));
        let mut host = AnalysisHost::new(None);
        host.apply_change(change);

        let tag = TextSize::from(class_text.find("tag").unwrap() as u32);
        let before = lookup_at(&host, class_file, tag).expect("tag before the edit");
        assert_eq!(before.owner_class, "holder");

        let mut edit = Change::new();
        edit.add_changed_file(ChangedFile::modify(
            module_file,
            "module b;\n  wire w;\nendmodule\n",
        ));
        host.apply_change(edit);

        let after = lookup_at(&host, class_file, tag).expect("tag after the edit");
        assert_eq!(after, before, "an unrelated edit must not change this answer");
    }
}
