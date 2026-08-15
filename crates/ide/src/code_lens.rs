use hir_def::{body::Body, def_id::DefId, has_source::HasSource, source_map::Lowered};

use preproc_expand::file::HirFileId;
use syntax::{
    ast::{self, AstNode},
    has_text_range::HasTextRangeIn,
};
use utils::text_edit::TextRange;
use vfs::FileId;

use crate::{
    FilePosition, FileRange, ScopeVisibility,
    db::root_db::RootDb,
    references::{
        ReferencesConfig,
        search::{ReferencesCtx, SearchScope},
    },
};

pub struct CodeLensConfig {
    pub instantiations: bool,
}

pub struct CodeLens {
    pub range: TextRange,
    pub kind: CodeLensKind,
}

pub enum CodeLensKind {
    ModuleInstance { pos: FilePosition, data: Option<Vec<FileRange>> },
}

pub(crate) fn code_lens(db: &RootDb, config: CodeLensConfig, file_id: FileId) -> Vec<CodeLens> {
    if db.file_kind(file_id).is_project_manifest() {
        return Vec::new();
    }
    let file_id = HirFileId::File(file_id);
    let hir_file =
        db.body_with_source_map(db.owner_table(file_id).file_owner().expect("file owner"));

    let mut res = Vec::new();

    if config.instantiations {
        process_instantiations(db, &hir_file, file_id, &mut res);
    }

    res
}

fn process_instantiations(
    db: &RootDb,
    hir_file: &Lowered<Body>,
    file_id: HirFileId,
    res: &mut Vec<CodeLens>,
) {
    for module_id in hir_file.module_owners() {
        let module = db.body_with_source_map(module_id);
        if module.name.is_none() {
            continue;
        }
        let Some(source) = module_id.source(db) else {
            continue;
        };
        let range = source.value.full_range();
        let pos = FilePosition { file_id: file_id.expect_file(), offset: range.start() };

        res.push(CodeLens { range, kind: CodeLensKind::ModuleInstance { pos, data: None } });
    }
}

pub(crate) fn code_lens_resolve(db: &RootDb, mut kind: CodeLensKind) -> CodeLensKind {
    let sema = db.semantics();

    match kind {
        CodeLensKind::ModuleInstance { pos: FilePosition { file_id, offset }, ref mut data } => {
            let hir_file_id = HirFileId::File(file_id);
            let hir_file = sema.db.body_with_source_map(
                sema.db.owner_table(hir_file_id).file_owner().expect("file owner"),
            );
            let Some(module_id) = hir_file.module_owners().find(|id| {
                id.source(db).is_some_and(|source| source.value.full_range().start() == offset)
            }) else {
                *data = Some(Vec::new());
                return kind;
            };
            let def =
                DefId::from_owner(sema.db, module_id).expect("module owner must have a definition");

            let ref_config =
                ReferencesConfig::new(ScopeVisibility::Public, Some(SearchScope::all(sema.db)));

            let mut ranges = Vec::new();
            for (file_id, tokens) in ReferencesCtx::new(&sema, &def, ref_config).search() {
                let parsed_file = sema.parse_file(file_id);
                for instantiation in tokens
                    .into_iter()
                    .filter_map(|tok| tok.to_token(parsed_file.syntax_tree()))
                    .filter_map(|tok| ast::HierarchyInstantiation::cast(tok.parent))
                {
                    for instance in instantiation.instances().children() {
                        if let Some(range) = instance.decl().and_then(|decl| {
                            decl.name().and_then(|name| name.text_range_in(decl.syntax()))
                        }) {
                            ranges.push(FileRange { file_id, range });
                        }
                    }
                }
            }

            *data = Some(ranges);
        }
    }

    kind
}
