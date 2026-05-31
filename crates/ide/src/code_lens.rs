use hir::{
    db::HirDb,
    file::HirFileId,
    hir_def::{
        file::{FileSourceMap, HirFile},
        module::ModuleId,
    },
    semantics::{Semantics, pathres::PathResolution},
    source_map::IsSrc,
};
use syntax::{
    ast::{self, AstNode},
    has_text_range::HasTextRangeIn,
};
use utils::{get::Get, text_edit::TextRange};
use vfs::FileId;

use crate::{
    FilePosition, FileRange, ScopeVisibility,
    db::root_db::RootDb,
    definitions::Definition,
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
    let file_id = HirFileId(file_id);
    let (hir_file, src_map) = db.hir_file_with_source_map(file_id);
    let (hir_file, src_map) = (hir_file.as_ref(), src_map.as_ref());

    let mut res = Vec::new();

    if config.instantiations {
        process_instantiations(hir_file, src_map, file_id, &mut res);
    }

    res
}

fn process_instantiations(
    hir_file: &HirFile,
    src_map: &FileSourceMap,
    file_id: HirFileId,
    res: &mut Vec<CodeLens>,
) {
    for (local_module_id, module_info) in hir_file.modules.iter() {
        if module_info.name.is_none() {
            continue;
        };

        let Some(range) = src_map.get(local_module_id).map(|src| src.range()) else {
            continue;
        };
        let pos = FilePosition { file_id: file_id.file_id(), offset: range.start() };

        res.push(CodeLens { range, kind: CodeLensKind::ModuleInstance { pos, data: None } });
    }
}

pub(crate) fn code_lens_resolve(db: &RootDb, mut kind: CodeLensKind) -> CodeLensKind {
    let sema = Semantics::new(db);

    match kind {
        CodeLensKind::ModuleInstance { pos: FilePosition { file_id, offset }, ref mut data } => {
            let hir_file_id = HirFileId(file_id);
            let (_, src_map) = sema.db.hir_file_with_source_map(hir_file_id);
            let Some((local_module_id, _)) =
                src_map.module_srcs.iter().find(|(_, src)| src.range().start() == offset)
            else {
                *data = Some(Vec::new());
                return kind;
            };
            let module_id = ModuleId::new(hir_file_id, local_module_id);

            let def = Definition(PathResolution::Module(module_id));

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
