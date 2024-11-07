use hir::{container::InFile, semantics::Semantics};
use ide_db::root_db::RootDb;
use line_index::TextRange;
use span::FilePosition;
use syntax::{SyntaxNodeExt, SyntaxTokenWithParent, TokenKind, ast::AstNode, token::TokenKindExt};
use vfs::FileId;

use crate::{
    ScopeVisibility,
    definitions::{Definition, DefinitionClass},
    references::{
        self, ReferenceCategory, ReferencesConfig,
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
    let file = sema.parse(file_id);

    let token = file.syntax().token_at_offset(offset).pick_bext_token(token_precedence)?;

    handle_ctrl_flow_kw(&sema, token).or_else(|| {
        let def = match DefinitionClass::resolve(&sema, token)? {
            DefinitionClass::Definition(def) => def,
            DefinitionClass::PortConnShorthand { data, .. } => data,
        };
        highlight_refs(&sema, file_id, def, config)
    })
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
    tp: SyntaxTokenWithParent,
) -> Option<Vec<DocumentHighlight>> {
    let cur_file_id = sema.find_file(tp.parent).file_id();
    let highlights = references::handle_ctrl_flow_kw(sema, tp)?
        .into_iter()
        .filter_map(|mut r| r.refs.remove(&cur_file_id))
        .flatten()
        .map(|(range, category)| DocumentHighlight { range, category })
        .collect();
    Some(highlights)
}

fn highlight_refs<'a>(
    sema: &'a Semantics<'a, RootDb>,
    file_id: FileId,
    def: Definition,
    DocumentHighlightConfig { scope_visibility }: DocumentHighlightConfig,
) -> Option<Vec<DocumentHighlight>> {
    let defs = def.origins().into_iter().filter_map(|def| {
        let InFile { value: (_, range), file_id: def_file_id } = def.name(sema.db);
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
        .remove(&file_id)?
        .into_iter()
        .map(|tok| DocumentHighlight { range: tok.range(), category: tok.category() });

    Some(defs.chain(refs).collect())
}
