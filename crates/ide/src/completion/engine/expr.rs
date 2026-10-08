use std::collections::BTreeMap;

use hir_def::{
    container::ScopeParent,
    def_id::DefId,
    owner::OwnerId,
    symbol::{DefKind, Resolution},
};
use hir_semantics::semantics::Semantics;
use preproc_expand::file::HirFileId;
use syntax::{SyntaxNode, SyntaxNodeExt};
use utils::text_edit::TextSize;

use super::{candidate::CompletionCandidate, system};
use crate::{
    FilePosition, analysis::AnalysisContext, completion::context::CompletionContext,
    db::root_db::RootDb,
};

#[derive(Clone, Debug)]
enum NameKind {
    Value,
    SubroutineCall,
}

pub(super) fn complete_expression(
    db: &AnalysisContext<'_>,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
    complete_expression_impl(db, position, prefix, ctx)
}

pub(super) fn complete_argument_exprs(
    db: &AnalysisContext<'_>,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
    complete_expression_impl(db, position, prefix, ctx)
}

fn complete_expression_impl(
    db: &AnalysisContext<'_>,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
    let sema = db.semantics();
    let file_id = position.file_id.into();
    let parsed_file = sema.parse_file(position.file_id);
    let Some(root) = parsed_file.root() else {
        return Vec::new();
    };

    let mut names: BTreeMap<String, NameKind> = BTreeMap::new();

    if let Some(container_id) = container_id_at_offset(&sema, file_id, root, position.offset) {
        for container_id in ScopeParent::start_from(db.db, container_id) {
            collect_container_names(db, container_id, &mut names);
        }
    }

    let mut candidates: Vec<_> = names
        .into_iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .map(|(name, kind)| match kind {
            NameKind::Value => CompletionCandidate::text(name, ctx.replacement),
            NameKind::SubroutineCall => CompletionCandidate::semantic_snippet(
                name.clone(),
                ctx.replacement,
                format!("{name}()"),
                format!("{name}(${{1:args}})"),
            ),
        })
        .collect();
    candidates.extend(system::complete_system_functions(prefix, ctx));
    candidates
}

fn container_id_at_offset(
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    root: SyntaxNode<'_>,
    offset: TextSize,
) -> Option<OwnerId> {
    let elem = root.covering_element(utils::line_index::TextRange::empty(offset));
    let node = elem.as_node().or_else(|| elem.parent())?;
    sema.container_for_node(file_id, node)
}

fn collect_container_names(
    db: &AnalysisContext<'_>,
    owner: OwnerId,
    names: &mut BTreeMap<String, NameKind>,
) {
    let scope = db.scope(owner);
    for (ident, defs) in scope.iter_listing() {
        collect_def_names(db, ident, defs, names);
    }
}

fn collect_def_names(
    db: &AnalysisContext<'_>,
    ident: &hir_def::Ident,
    defs: impl IntoIterator<Item = DefId>,
    names: &mut BTreeMap<String, NameKind>,
) {
    let defs = defs.into_iter().collect::<Vec<_>>();

    let subroutines = Resolution::from_candidates(
        defs.iter().filter_map(|def_id| def_id.primary_origin(db.db).as_subroutine(db.db)),
    );
    if !matches!(subroutines, Resolution::Unresolved) {
        names.entry(ident.to_string()).or_insert(NameKind::SubroutineCall);
        return;
    }

    if defs.iter().any(|def_id| {
        matches!(
            def_id.kind(db.db),
            DefKind::Variable
                | DefKind::Net
                | DefKind::Param
                | DefKind::Port
                | DefKind::Genvar
                | DefKind::Specparam
                | DefKind::SubroutinePort
        )
    }) {
        names.entry(ident.to_string()).or_insert(NameKind::Value);
    }
}
