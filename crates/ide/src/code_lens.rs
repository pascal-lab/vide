use hir_def::{db::HirDefDb, def_id::DefId, module::ModuleId};
use hir_semantics::semantics::Semantics;
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
    let sema = hir::Semantics::new(db);
    let file = hir::File::from(file_id);

    let mut res = Vec::new();

    if config.instantiations {
        for (source, module) in sema.module_declarations(file) {
            if module.name(db).is_none() {
                continue;
            }
            let pos = FilePosition {
                file_id: source.file().expect_file(),
                offset: source.range().start(),
            };
            res.push(CodeLens {
                range: source.range(),
                kind: CodeLensKind::ModuleInstance { pos, data: None },
            });
        }
    }

    res
}

pub(crate) fn code_lens_resolve(db: &RootDb, mut kind: CodeLensKind) -> CodeLensKind {
    let sema = Semantics::new(db);

    match kind {
        CodeLensKind::ModuleInstance { pos: FilePosition { file_id, offset }, ref mut data } => {
            let hir_file_id = HirFileId::File(file_id);
            let hir_file = sema.db.hir_file_with_source_map(hir_file_id);
            let Some((local_module_id, _)) = hir_file.modules.iter().find(|(id, _)| {
                hir_file.source_range(*id).is_some_and(|range| range.start() == offset)
            }) else {
                *data = Some(Vec::new());
                return kind;
            };
            let module_id = ModuleId::new(hir_file_id, local_module_id);

            let def = DefId::new(sema.db, module_id);

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
