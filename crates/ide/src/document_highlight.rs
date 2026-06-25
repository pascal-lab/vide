use hir::{container::InFile, file::HirFileId, semantics::Semantics};
use syntax::{SyntaxTokenWithParent, has_text_range::HasTextRange, token::pair_token};
use utils::line_index::TextRange;
use vfs::FileId;

use crate::{
    FilePosition, ScopeVisibility,
    db::root_db::RootDb,
    definitions::{Definition, DefinitionClass},
    facts::{
        SemanticFacts, TargetQuery,
        target::{SemanticTarget, TargetIntent},
    },
    references::{
        ReferenceCategory, ReferencesConfig,
        search::{ReferencesCtx, SearchScope},
    },
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
    let target = SemanticFacts::new(db).target_at(TargetQuery {
        file_id,
        offset,
        intent: TargetIntent::Highlight,
        root: parsed_file.root(),
    });
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

fn handle_ctrl_flow_kw(
    tp @ SyntaxTokenWithParent { .. }: SyntaxTokenWithParent,
) -> Option<Vec<DocumentHighlight>> {
    let pair = pair_token(tp)?;
    let pair = pair.either(|token| token, |token| token);
    let highlights = [tp, pair]
        .into_iter()
        .filter_map(|token| {
            Some(DocumentHighlight {
                range: token.text_range()?,
                category: ReferenceCategory::empty(),
            })
        })
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
    handle_ctrl_flow_kw(token).or_else(|| {
        let def = match DefinitionClass::resolve(sema, hir_file_id, token)? {
            DefinitionClass::Definition(def) => def,
            DefinitionClass::PortConnShorthand { local, .. } => local,
            DefinitionClass::Ambiguous(_) => return None,
        };
        highlight_refs(sema, file_id, def, config)
    })
}

fn highlight_refs<'a>(
    sema: &'a Semantics<'a, RootDb>,
    file_id: FileId,
    def: Definition,
    DocumentHighlightConfig { scope_visibility }: DocumentHighlightConfig,
) -> Option<Vec<DocumentHighlight>> {
    let defs = def.origins().into_iter().filter_map(|def| {
        let InFile { value: range, file_id: def_file_id } = def.name_range(sema.db)?;
        if file_id == def_file_id.file_id() {
            Some(DocumentHighlight { range, category: ReferenceCategory::empty() })
        } else {
            None
        }
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

    use hir::base_db::{change::Change, source_root::SourceRoot};
    use insta::assert_debug_snapshot;
    use triomphe::Arc;
    use utils::{lines::LineEnding, text_edit::TextSize};
    use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

    use super::*;
    use crate::{ScopeVisibility, analysis_host::AnalysisHost, test_utils::normalize_fixture_text};

    fn setup(text: &str) -> (AnalysisHost, FilePosition) {
        let text = normalize_fixture_text(text);
        let marker = "/*caret*/";
        let off = text.find(marker).expect("missing /*caret*/");
        let mut owned = text;
        owned = owned.replace(marker, "");

        let file_id = FileId(0);
        let path = VfsPath::new_virtual_path("/test.v".to_string());

        let mut file_set = FileSet::default();
        file_set.insert(file_id, path);
        let root = SourceRoot::new_local(file_set);

        let mut change = Change::new();
        change.set_roots(vec![root]);
        change.add_changed_file(ChangedFile {
            file_id,
            change_kind: ChangeKind::Create(Arc::from(owned.as_str()), LineEnding::Unix),
        });

        let mut host = AnalysisHost::default();
        host.apply_change(change);
        let position = FilePosition { file_id, offset: TextSize::from(off as u32) };
        (host, position)
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
