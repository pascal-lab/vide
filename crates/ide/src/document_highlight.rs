use hir::{container::InFile, def_id::DefId, file::HirFileId, semantics::Semantics};
use syntax::{SyntaxTokenWithParent, TokenKind, token::TokenKindExt};
use utils::line_index::TextRange;
use vfs::FileId;

use crate::{
    FilePosition, ScopeVisibility,
    db::root_db::RootDb,
    definitions::DefinitionClass,
    references::{
        self, ReferenceCategory, ReferencesConfig,
        search::{ReferencesCtx, SearchScope, resolve_source_range},
    },
    semantic_target::{SemanticTarget, TargetIntent, resolve_semantic_target},
};

#[derive(Debug, Clone)]
pub struct DocumentHighlightConfig {
    pub scope_visibility: ScopeVisibility,
}

#[derive(Debug, Clone)]
pub struct DocumentHighlight {
    pub range: TextRange,
    pub category: ReferenceCategory,
}

pub(crate) fn document_highlight(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
    config: DocumentHighlightConfig,
) -> Option<Vec<DocumentHighlight>> {
    let sema = Semantics::new(db);
    let hir_file_id = file_id.into();
    let parsed_file = sema.parse_file(file_id);
    let target = resolve_semantic_target(db, file_id, offset, parsed_file.root(), token_precedence);
    let SemanticTarget::Source(target) = target.unique_for_intent(TargetIntent::Highlight)? else {
        return None;
    };
    let tokens = target.into_tokens();
    let highlights = tokens
        .into_iter()
        .filter_map(|token| highlight_for_token(&sema, file_id, hir_file_id, token, config.clone()))
        .flatten()
        .collect::<Vec<_>>();
    (!highlights.is_empty()).then_some(highlights)
}

fn token_precedence(kind: TokenKind) -> usize {
    match kind {
        _ if kind.name_like() => 4,
        _ if kind.is_pair_token() => 4,
        _ => 1,
    }
}

fn handle_ctrl_flow_kw(
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    tp: SyntaxTokenWithParent,
) -> Option<Vec<DocumentHighlight>> {
    let cur_file_id = file_id.expect_file();
    let highlights = references::handle_ctrl_flow_kw(sema, file_id, tp)?
        .into_iter()
        .filter_map(|mut r| r.refs.remove(&cur_file_id))
        .flatten()
        .map(|(range, category)| DocumentHighlight { range, category })
        .collect();
    Some(highlights)
}

fn highlight_for_token(
    sema: &Semantics<'_, RootDb>,
    file_id: FileId,
    hir_file_id: HirFileId,
    token: SyntaxTokenWithParent,
    config: DocumentHighlightConfig,
) -> Option<Vec<DocumentHighlight>> {
    handle_ctrl_flow_kw(sema, hir_file_id, token).or_else(|| {
        let def = match DefinitionClass::resolve(sema, hir_file_id, token).unique()? {
            DefinitionClass::Definition(def) => def,
            DefinitionClass::PortConnShorthand { local, .. } => local,
        };
        highlight_refs(sema, file_id, def, config)
    })
}

fn highlight_refs<'a>(
    sema: &'a Semantics<'a, RootDb>,
    file_id: FileId,
    def: DefId,
    DocumentHighlightConfig { scope_visibility }: DocumentHighlightConfig,
) -> Option<Vec<DocumentHighlight>> {
    let defs = def.origins(sema.db).into_iter().filter_map(|def| {
        let InFile { value: range, file_id: def_file_id } = def.name_range(sema.db)?;
        let (def_file_id, range) = resolve_source_range(sema.db, def_file_id, range)?;
        (def_file_id == file_id)
            .then_some(DocumentHighlight { range, category: ReferenceCategory::empty() })
    });

    let ref_config =
        ReferencesConfig::new(scope_visibility, Some(SearchScope::single_file(file_id)));
    let refs = ReferencesCtx::new(sema, &def, ref_config)
        .search()
        .remove(&file_id)
        .unwrap_or_default()
        .into_iter()
        .map(|tok| DocumentHighlight { range: tok.range(), category: tok.category() });

    Some(defs.chain(refs).collect())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use hir::{
        base_db::{change::Change, source_root::SourceRoot},
        db::HirDb,
        hir_def::macro_file::macro_files_at_offset,
    };
    use insta::assert_debug_snapshot;
    use utils::text_edit::TextSize;
    use vfs::{ChangedFile, FileId, FileSet, VfsPath};

    use super::*;
    use crate::{ScopeVisibility, analysis_host::AnalysisHost, test_utils::normalize_fixture_text};

    fn setup(text: &str) -> (AnalysisHost, FilePosition) {
        let text = normalize_fixture_text(text);
        let marker = "/*caret*/";
        let off = text.find(marker).expect("missing /*caret*/");
        let mut owned = text;
        owned = owned.replace(marker, "");

        let file_id = FileId::from_raw(0);
        let path = VfsPath::new_virtual_path("/test.v".to_string());

        let mut file_set = FileSet::default();
        file_set.insert(file_id, path);
        let root = SourceRoot::new_local(file_set);

        let mut change = Change::new();
        change.set_roots(vec![root]);
        change.add_changed_file(ChangedFile::create(file_id, owned.as_str()));

        let mut host = AnalysisHost::default();
        host.apply_change(change);
        let position = FilePosition { file_id, offset: TextSize::from(off as u32) };
        (host, position)
    }

    #[test]
    fn macro_generated_definition_highlight_uses_call_site_range() {
        let text = r#"
`define DECL module generated; endmodule
`DECL

module top;
  /*caret*/generated u();
endmodule
"#;
        let clean_text = normalize_fixture_text(text).replace("/*caret*/", "");
        let call_start = clean_text.find("`DECL\n").expect("macro call");
        let call_range = TextRange::new(
            TextSize::from(call_start as u32),
            TextSize::from((call_start + "`DECL".len()) as u32),
        );
        let (host, position) = setup(text);
        let db = host.raw_db();
        let macro_file =
            macro_files_at_offset(db, position.file_id, TextSize::from(call_start as u32))
                .pop()
                .expect("macro expansion");
        let hir_file_id = HirFileId::Macro(macro_file);
        let (hir_file, _) = db.hir_file_with_source_map(hir_file_id);
        let (local_module_id, _) = hir_file.modules.iter().next().expect("macro-generated module");
        let def = DefId::new(db, InFile::new(hir_file_id, local_module_id));

        let highlights = highlight_refs(
            &Semantics::new(db),
            position.file_id,
            def,
            DocumentHighlightConfig { scope_visibility: ScopeVisibility::Public },
        )
        .expect("module highlights");

        let mut ranges =
            highlights.into_iter().map(|highlight| highlight.range).collect::<Vec<_>>();
        ranges.sort_unstable_by_key(|range| (range.start(), range.end()));
        assert_eq!(ranges, vec![call_range], "highlights must contain only source-document ranges");
    }

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/document_highlight/fixtures")
    }

    #[test]
    fn document_highlight_fixtures() {
        let dir = fixtures_dir();
        let mut fixtures: Vec<(String, PathBuf)> = std::fs::read_dir(&dir)
            .unwrap_or_else(|err| panic!("failed to read fixtures dir {dir:?}: {err}"))
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.extension()? != "v" {
                    return None;
                }
                let name = path.file_stem()?.to_string_lossy().to_string();
                Some((name, path))
            })
            .collect();

        fixtures.sort_by(|a, b| a.0.cmp(&b.0));
        assert!(!fixtures.is_empty(), "no fixtures found in {dir:?}");

        for (name, path) in fixtures {
            let text =
                std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {path:?}: {err}"));
            let text = normalize_fixture_text(&text);
            let (host, position) = setup(&text);
            let highlights = host
                .make_analysis()
                .document_highlight(
                    position,
                    DocumentHighlightConfig { scope_visibility: ScopeVisibility::Public },
                )
                .unwrap()
                .unwrap_or_default();
            assert_debug_snapshot!(name, highlights);
        }
    }
}
