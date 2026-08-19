//! Record generated CU units from an already-paid compilation artifact.
//!
//! Does not parse. Callers must have already computed
//! `compilation_unit_artifact` for `file_id` (parse_file / include-edge
//! dependency recording). The next catalog read merges this overlay; it
//! does not patch a handwritten graph.

use design_graph::{
    FileFacts, UnitId, UnitMeta, UnitOrigin,
    facts::extract::{cu_unit_names, unit_fingerprint},
};
use preproc_expand::db::PreprocDb;
use rustc_hash::FxHashMap;
use syntax::preproc::{TokenOrigin, Trace};
use vfs::FileId;

use crate::analysis::AnalysisContext;

pub(crate) fn record_from_paid_artifact(db: &AnalysisContext<'_>, file_id: FileId) {
    let fingerprint = <dyn PreprocDb>::compilation_unit_snapshot(db.db, file_id).fingerprint;
    let Some(trace) = db.preproc_trace(file_id) else {
        db.store.record_generated_units(file_id, fingerprint, Box::new([]), FxHashMap::default());
        return;
    };
    let tree = db.parse_tree(file_id);
    let facts = db.file_facts(file_id);
    let (ids, meta) = collect_generated_units(file_id, &tree, &trace, &facts);
    db.store.record_generated_units(file_id, fingerprint, ids, meta);
}

fn collect_generated_units(
    file_id: FileId,
    tree: &syntax::SyntaxTree,
    trace: &Trace,
    unexpanded: &FileFacts,
) -> (Box<[UnitId]>, FxHashMap<UnitId, UnitMeta>) {
    let mut next_ordinal = FxHashMap::default();
    for unit in unexpanded.units.iter() {
        next_ordinal.insert((unit.id.name.clone(), unit.id.kind), unit.id.ordinal + 1);
    }
    let mut ids = Vec::new();
    let mut meta = FxHashMap::default();
    for header in cu_unit_names(tree) {
        let Some(index) = header.emitted else {
            continue;
        };
        let Some(origin) = origin_at(trace, index) else {
            continue;
        };
        if matches!(origin, TokenOrigin::Source { .. }) {
            continue;
        }
        let ordinal = next_ordinal.entry((header.name.clone(), header.kind)).or_insert(0);
        let id = UnitId {
            file: file_id,
            name: header.name.clone(),
            kind: header.kind,
            ordinal: *ordinal,
        };
        *ordinal += 1;
        meta.insert(
            id.clone(),
            UnitMeta {
                kind: header.kind,
                origin: UnitOrigin::Generated,
                header_fingerprint: unit_fingerprint(header.kind, &header.name),
            },
        );
        ids.push(id);
    }
    (ids.into_boxed_slice(), meta)
}

fn origin_at(trace: &Trace, index: u32) -> Option<&TokenOrigin> {
    let token = trace.emitted_tokens.get(usize::try_from(index).ok()?)?;
    debug_assert!(token.emitted_token_index.is_none_or(|got| got == index));
    Some(&token.origin)
}

#[cfg(test)]
mod tests {
    use crate::test_utils::setup;

    #[test]
    fn unpaid_file_has_no_generated_entry() {
        let (host, file_id) = setup("module top;\nendmodule\n");
        let generated = host.ctx().store.generated_units(host.ctx().db);
        assert!(!generated.contains_file(file_id), "{generated:?}");
    }

    #[test]
    fn source_visible_module_is_not_recorded_as_generated() {
        let (host, file_id) = setup("module top;\nendmodule\n");
        let ctx = host.ctx();
        let _ = ctx.parse_file(file_id);
        let generated = ctx.store.generated_units(ctx.db);
        assert!(
            generated.contains_file(file_id) && generated.ids_for(file_id).is_empty(),
            "{generated:?}"
        );
    }

    #[test]
    fn empty_scan_is_idempotent() {
        let (host, file_id) = setup("module top;\nendmodule\n");
        let ctx = host.ctx();
        let _ = ctx.parse_file(file_id);
        let first = ctx.store.generated_units(ctx.db);
        let _ = ctx.parse_file(file_id);
        let second = ctx.store.generated_units(ctx.db);
        assert_eq!(first, second);
    }

    #[test]
    fn macro_generated_module_is_recorded() {
        let text = "`define GEN(name) module name; endmodule\n`GEN(foo)\nmodule top;\nendmodule\n";
        let (host, file_id) = setup(text);
        let ctx = host.ctx();
        let facts = ctx.file_facts(file_id);
        assert!(
            facts.units.iter().all(|unit| unit.id.name != "foo"),
            "unexpanded facts must not invent the generated name: {:?}",
            facts.units
        );
        let _ = ctx.parse_file(file_id);
        let generated = ctx.store.generated_units(ctx.db);
        assert!(generated.contains_file(file_id), "{generated:?}");
        let ids = generated.ids_for(file_id);
        assert_eq!(ids.len(), 1, "{generated:?}");
        assert_eq!(ids[0].name, "foo");
        assert_eq!(ids[0].kind, design_graph::UnitKind::Module);
        assert_eq!(ids[0].ordinal, 0);
        assert_eq!(generated.meta[&ids[0]].origin, design_graph::UnitOrigin::Generated);
        assert!(facts.units.iter().any(|unit| unit.id.name == "top"));
        assert!(ids.iter().all(|id| id.name != "top"));
    }

    #[test]
    fn generated_ordinal_continues_after_source_units() {
        let text = "`define GEN(name) module name; endmodule\n`GEN(top)\nmodule top;\nendmodule\n";
        let (host, file_id) = setup(text);
        let ctx = host.ctx();
        let facts = ctx.file_facts(file_id);
        assert_eq!(facts.units.len(), 1);
        assert_eq!(facts.units[0].id.name, "top");
        assert_eq!(facts.units[0].id.ordinal, 0);
        let _ = ctx.parse_file(file_id);
        let generated = ctx.store.generated_units(ctx.db);
        assert!(generated.contains_file(file_id), "{generated:?}");
        let ids = generated.ids_for(file_id);
        assert_eq!(ids.len(), 1, "{generated:?}");
        assert_eq!(ids[0].name, "top");
        assert_eq!(ids[0].ordinal, 1);
    }
}
