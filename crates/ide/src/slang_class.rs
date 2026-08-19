//! T4 slang FFI slice: class-member type / owner / inheritance.
//!
//! The slice is in-process. A missing answer is `None` (HIR hover still
//! works). That is the "service down → drop fidelity, not function" rule.

use base_db::source_db::SourceRootDb;
use hir_def::ast_id_map::SourceAstId;
use preproc_expand::file::HirFileId;
use slang_sys::compilation::{ClassMemberInfo, Compilation};
use syntax::{SyntaxTreeOptions, has_text_range::HasTextRange};
use vfs::FileId;

use crate::db::root_db::RootDb;

/// Look up a class member in `text` at `offset` via a fresh slang compilation.
pub fn lookup_in_text(
    text: &str,
    name: &str,
    path: &str,
    offset: usize,
    include_paths: &[String],
) -> Option<ClassMemberInfo> {
    let mut compilation = Compilation::new();
    let options =
        SyntaxTreeOptions { include_paths: include_paths.to_vec(), ..SyntaxTreeOptions::default() };
    compilation.parse_syntax_tree_from_text(text, name, path, &options);
    compilation.lookup_class_member(path, offset)
}

/// Shipped `(FileId, SourceAstId)` entry: map the stable id to a range, then
/// ask slang on the same text.
pub fn lookup_from_ast_id(
    db: &RootDb,
    file_id: FileId,
    ast_id: SourceAstId,
) -> Option<ClassMemberInfo> {
    let hir_file = HirFileId::File(file_id);
    let tree = db.parse(hir_file);
    let map = db.ast_id_map(hir_file);
    let node = map.node(ast_id, &tree)?;
    let offset = usize::from(node.text_range()?.start());
    let text = db.file_text(file_id);
    let path = db
        .file_path(file_id)
        .map(|path| path.to_string())
        .unwrap_or_else(|| format!("file{}", file_id.index()));
    let name = path.clone();
    lookup_in_text(&text, &name, &path, offset, &[])
}

pub fn format_answer(info: &ClassMemberInfo) -> String {
    let mut line = format!("{} :: {}", info.owner_class, info.type_name);
    if !info.inheritance.is_empty() {
        line.push_str(" extends ");
        line.push_str(&info.inheritance.join(" > "));
    }
    line
}

/// Independent `SourceAstId` computation on two parses of the same text.
/// This is the §3.7 check: same text + same options ⇒ same stable paths.
#[cfg(test)]
fn source_ast_ids_agree(text: &str, name: &str, path: &str) -> (usize, usize) {
    let options = SyntaxTreeOptions::without_include_expansion();
    let tree_a = syntax::SyntaxTree::from_file_in_memory_with_options(text, name, path, &options);
    let tree_b = syntax::SyntaxTree::from_file_in_memory_with_options(text, name, path, &options);
    let map_a = hir_def::ast_id_map::AstIdMap::from_source(&tree_a);
    let map_b = hir_def::ast_id_map::AstIdMap::from_source(&tree_b);
    let ids = |tree: &syntax::SyntaxTree, map: &hir_def::ast_id_map::AstIdMap| {
        let mut ids = Vec::new();
        for event in tree.root().node_preorder() {
            let syntax::WalkEvent::Enter(node) = event else {
                continue;
            };
            ids.push(map.id_of_node(node));
        }
        ids
    };
    let a = ids(&tree_a, &map_a);
    let b = ids(&tree_b, &map_b);
    let compared = a.len().min(b.len());
    let matched = a.iter().zip(&b).filter(|(left, right)| left == right).count();
    (matched, compared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{position, setup_marked};

    const UVM_OBJECT: &str = r#"
virtual class uvm_void;
endclass
virtual class uvm_object extends uvm_void;
  string /*marker:name*/m_leaf_name;
  function string get_type_name();
    return "";
  endfunction
endclass
"#;

    #[test]
    fn shipped_lookup_from_ast_id_returns_class_member() {
        let src = "virtual class uvm_void; endclass\nvirtual class uvm_object extends uvm_void;\n  string m_leaf_name;\nendclass\n";
        let (host, file_id) = crate::test_utils::setup_with_path(src, "/uvm_object.svh");
        let tree = host.ctx().parse_file(file_id);
        let map = host.ctx().db.ast_id_map(HirFileId::File(file_id));
        let mut found = None;
        for event in tree.root().node_preorder() {
            let syntax::WalkEvent::Enter(node) = event else {
                continue;
            };
            let Some(range) = node.text_range() else {
                continue;
            };
            let start = usize::from(range.start());
            let end = usize::from(range.end());
            if !src.get(start..end).is_some_and(|span| span.contains("m_leaf_name")) {
                continue;
            }
            if let Some(id) = map.id_of_node(node) {
                found = lookup_from_ast_id(host.ctx().db, file_id, id);
                if found.is_some() {
                    break;
                }
            }
        }
        let info = found.expect("shipped (FileId, SourceAstId) path must hit slang");
        assert_eq!(info.owner_class, "uvm_object");
        assert!(info.inheritance.iter().any(|name| name == "uvm_void"), "{info:?}");
        assert!(info.type_name.contains("string"), "{info:?}");
    }

    #[test]
    fn hover_shows_slang_answer_beside_hir_ty() {
        let (host, file_id, _text, markers) = setup_marked(UVM_OBJECT);
        let hover = host.make_analysis().hover(position(file_id, &markers, "name")).unwrap();
        let markup = hover.expect("hover the UVM class type").info;
        let text = markup.as_str();
        assert!(
            text.contains("slang") && text.contains("uvm_object"),
            "hover must run slang beside hir-ty:\n{text}"
        );
    }

    #[test]
    fn section_3_7_ids_agree_on_independent_parses() {
        let (matched, compared) =
            source_ast_ids_agree(UVM_OBJECT, "uvm_object.svh", "uvm_object.svh");
        assert!(compared > 0, "must compare at least one node");
        assert_eq!(matched, compared, "§3.7: {matched}/{compared} SourceAstId values matched");
    }

    #[test]
    fn t4_gate_numbers() {
        use std::time::Instant;

        let src = UVM_OBJECT;
        let offset = src.find("m_leaf_name").expect("property");
        let mut times = Vec::new();
        let mut hits = 0usize;
        for _ in 0..40 {
            let started = Instant::now();
            let info = lookup_in_text(src, "uvm_object.svh", "uvm_object.svh", offset, &[]);
            times.push(started.elapsed().as_secs_f64() * 1000.0);
            if info.is_some() {
                hits += 1;
            }
        }
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p95 = times[((times.len() * 95) / 100).min(times.len() - 1)];
        let (matched, compared) = source_ast_ids_agree(src, "uvm_object.svh", "uvm_object.svh");
        let id_ok = compared > 0 && matched == compared;
        let consistency_pct = 0.0;
        println!("t4.p95_ms\t{p95:.3}");
        println!("t4.consistency_vs_hir_ty\t{consistency_pct:.1}%");
        println!("t4.section_3_7\t{matched}/{compared} {}", if id_ok { "pass" } else { "fail" });
        println!("t4.slang_hits\t{hits}/{}", times.len());
        println!(
            "t4.gate\tp95<50ms={} consistency>99%={} §3.7={}",
            p95 < 50.0,
            consistency_pct > 99.0,
            id_ok
        );
        assert!(hits == times.len(), "slang must answer every UVM class-member query");
        assert!(id_ok, "§3.7 must hold");
    }
}
