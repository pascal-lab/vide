//! Semantic lookups through the resident elaboration service.
//!
//! These functions only turn an IDE position into the arguments slang wants
//! and hand back the whole [`ElabResult`]. Deciding what a non-answer means
//! is the caller's job, and every caller does it the same way, through
//! [`ElabResult::answered`], so "slang is down" never reads as "no such
//! symbol".
//!
//! [`ElabResult::answered`]: crate::elaboration::ElabResult::answered

use base_db::source_db::SourceRootDb;
use preproc_expand::compilation_plan;
use slang_sys::compilation::{MemberInfo, SymbolInfo};
#[cfg(test)]
use syntax::SyntaxTreeOptions;
use vfs::FileId;

use crate::{analysis::AnalysisContext, elaboration::ElabResult};

pub fn lookup_symbol_at(
    ctx: &AnalysisContext<'_>,
    file_id: FileId,
    offset: usize,
) -> ElabResult<SymbolInfo> {
    let path = compilation_plan::source_buffer_path(ctx.db, file_id).to_string();
    let profile = ctx.db.file_compilation_profile(file_id);
    ctx.elab.lookup_symbol(ctx.db, ctx.revision, profile, &path, offset)
}

pub fn lookup_scoped_at(
    ctx: &AnalysisContext<'_>,
    file_id: FileId,
    left: &str,
    right: &str,
) -> ElabResult<SymbolInfo> {
    let profile = ctx.db.file_compilation_profile(file_id);
    ctx.elab.lookup_scoped(ctx.db, ctx.revision, profile, left, right)
}

/// Members of the scope a name denotes: a package, a class, or a
/// hierarchical instance path such as `top.u0` or `u0[0]`.
///
/// Empty when `name` denotes no scope — including when it is an expression
/// rather than a name. Those belong to [`list_members_at`], which resolves
/// them at their own offset.
pub fn list_scope_members_at(
    ctx: &AnalysisContext<'_>,
    file_id: FileId,
    name: &str,
) -> ElabResult<Vec<MemberInfo>> {
    let profile = ctx.db.file_compilation_profile(file_id);
    ctx.elab.list_scope_members(ctx.db, ctx.revision, profile, name)
}

pub fn list_members_at(
    ctx: &AnalysisContext<'_>,
    file_id: FileId,
    offset: usize,
) -> ElabResult<Vec<MemberInfo>> {
    let path = compilation_plan::source_buffer_path(ctx.db, file_id).to_string();
    let profile = ctx.db.file_compilation_profile(file_id);
    ctx.elab.list_members(ctx.db, ctx.revision, profile, &path, offset)
}

pub fn lookup_type_at(
    ctx: &AnalysisContext<'_>,
    file_id: FileId,
    start: usize,
    end: usize,
) -> ElabResult<String> {
    let path = compilation_plan::source_buffer_path(ctx.db, file_id).to_string();
    let profile = ctx.db.file_compilation_profile(file_id);
    ctx.elab.lookup_type(ctx.db, ctx.revision, profile, &path, start, end)
}

/// `owner :: type extends base > base` for a class member.
pub fn format_class_member(owner_class: &str, type_name: &str, inheritance: &[String]) -> String {
    let mut line = format!("{owner_class} :: {type_name}");
    if !inheritance.is_empty() {
        line.push_str(" extends ");
        line.push_str(&inheritance.join(" > "));
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

    /// Wait for the build, then use the shipped entry point. Nothing here may
    /// fall back to a private compilation: a test that answers by a route
    /// production does not take proves nothing about production.
    fn shipped_symbol_at(
        host: &crate::analysis_host::AnalysisHost,
        file_id: FileId,
        offset: utils::line_index::TextSize,
    ) -> Option<SymbolInfo> {
        let ctx = host.ctx();
        let built = ctx.elab.prewarm(ctx.db, ctx.revision);
        assert!(matches!(built, ElabResult::Ready(_)), "build must finish, got {built:?}");
        match lookup_symbol_at(&ctx, file_id, usize::from(offset)) {
            ElabResult::Ready(info) => info,
            other => panic!("shipped lookup must be Ready, got {other:?}"),
        }
    }

    #[test]
    fn the_shipped_offset_entry_returns_the_class_member() {
        let (host, file_id, _text, markers) = setup_marked(UVM_OBJECT);
        let info =
            shipped_symbol_at(&host, file_id, markers["name"]).expect("class property is a symbol");
        assert_eq!(info.owner_class, "uvm_object");
        assert!(info.inheritance.iter().any(|name| name == "uvm_void"), "{info:?}");
        assert!(info.type_name.contains("string"), "{info:?}");
    }

    #[test]
    fn hover_shows_slang_type_for_a_net() {
        let src = "module top;\n  logic [7:0] /*marker:x*/x;\nendmodule\n";
        let (host, file_id, _text, markers) = setup_marked(src);
        let hover = host.make_analysis().hover(position(file_id, &markers, "x")).unwrap();
        let markup = hover.expect("net hover");
        let text = markup.info.as_str();
        assert!(text.contains("logic"), "net hover must show the declaration type:\n{text}");
    }

    #[test]
    fn hover_shows_slang_type() {
        let (host, file_id, _text, markers) = setup_marked(UVM_OBJECT);
        let hover = host.make_analysis().hover(position(file_id, &markers, "name")).unwrap();
        let markup = hover.expect("hover the UVM class type").info;
        let text = markup.as_str();
        assert!(text.contains("string"), "class property hover must show the member type:\n{text}");
        assert!(!text.contains("hir-ty"), "TypeSystem is not the hover type answer:\n{text}");
    }

    #[test]
    fn class_scope_goto_is_answered_by_slang() {
        let src = r#"
class env;
  static int /*marker:def*/count;
endclass
module top;
  initial env::/*marker:use*/count = 1;
endmodule
"#;
        let (host, file_id, _text, markers) = setup_marked(src);
        let nav = host
            .make_analysis()
            .goto_definition(position(file_id, &markers, "use"))
            .unwrap()
            .expect("env::count");
        assert!(
            nav.info.iter().any(|target| target.focus_range.map(|range| range.start())
                == Some(markers["def"])),
            "class :: must jump to the member: {nav:?}"
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

        let (host, file_id, _text, markers) = setup_marked(UVM_OBJECT);
        let pos = position(file_id, &markers, "name");
        let slang =
            shipped_symbol_at(&host, file_id, markers["name"]).expect("slang answers the member");

        let mut times = Vec::new();
        let mut hits = 0usize;
        for _ in 0..40 {
            let started = Instant::now();
            let hover = host.make_analysis().hover(pos).unwrap();
            times.push(started.elapsed().as_secs_f64() * 1000.0);
            if hover.as_ref().is_some_and(|h| h.info.as_str().contains("string")) {
                hits += 1;
            }
        }
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p95 = times[((times.len() * 95) / 100).min(times.len() - 1)];

        let (matched, compared) =
            source_ast_ids_agree(UVM_OBJECT, "uvm_object.svh", "uvm_object.svh");
        let id_ok = compared > 0 && matched == compared;
        println!("t4.slang\t{}", slang.type_name);
        println!("t4.p95_ms\t{p95:.3}");
        println!("t4.section_3_7\t{matched}/{compared} {}", if id_ok { "pass" } else { "fail" });
        println!("t4.slang_hits\t{hits}/{}", times.len());
        println!("t4.gate\tp95<50ms={} §3.7={}", p95 < 50.0, id_ok);
        assert!(hits == times.len(), "hover must answer every request");
        assert!(id_ok, "§3.7 must hold");
    }
}
