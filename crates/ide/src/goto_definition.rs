use hir::{
    base_db::source_db::SourceDb,
    container::InFile,
    file::HirFileId,
    preproc::{
        IncludeTarget, MacroDefinition, include_directive_at, macro_definition_at,
        macro_reference_definitions_at,
    },
    semantics::Semantics,
};
use itertools::Itertools;
use syntax::{
    SyntaxNodeExt, SyntaxTokenWithParent, TokenKind,
    has_text_range::HasTextRange,
    token::{TokenKindExt, pair_token},
};
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use crate::{
    FilePosition, RangeInfo,
    db::root_db::RootDb,
    definitions::DefinitionClass,
    navigation_target::{NavTarget, ToNav},
};

pub(crate) fn goto_definition(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    if let Some(macro_definition) = handle_preproc_macro(db, file_id, offset) {
        return Some(macro_definition);
    }

    if let Some(include) = handle_preproc_include(db, file_id, offset) {
        return Some(include);
    }

    let sema = Semantics::new(db);
    let hir_file_id = file_id.into();
    let parsed_file = sema.parse_file(file_id);
    let root = parsed_file.root()?;
    let token = root.token_at_offset(offset).pick_bext_token(token_precedence)?;

    let navs = handle_ctrl_flow_kw(&sema, hir_file_id, token).or_else(|| {
        DefinitionClass::resolve(&sema, hir_file_id, token)?
            .origins()
            .into_iter()
            .unique()
            .filter_map(|def| def.to_nav(db))
            .collect_vec()
            .into()
    })?;

    Some(RangeInfo::new(token.text_range()?, navs))
}

fn handle_preproc_macro(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    if let Some(definition) = macro_definition_at(db, file_id, offset).ok()? {
        return Some(RangeInfo::new(definition.name_range, vec![macro_nav_target(definition)]));
    }

    let resolution = macro_reference_definitions_at(db, file_id, offset).ok()??;
    let reference_range = resolution.reference.range;
    let targets = resolution.definitions.into_iter().map(macro_nav_target).collect_vec();
    if targets.is_empty() {
        return None;
    }
    Some(RangeInfo::new(reference_range, targets))
}

fn macro_nav_target(definition: MacroDefinition) -> NavTarget {
    NavTarget {
        file_id: definition.file_id,
        full_range: definition.name_range,
        focus_range: Some(definition.name_range),
        name: Some(definition.name),
        kind: None,
        container_name: None,
        description: Some("macro definition".to_owned()),
    }
}

fn handle_preproc_include(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let include = include_directive_at(db, file_id, offset).ok()??;
    let IncludeTarget::Literal { path, resolved_file: Some(target_file_id) } = include.target
    else {
        return None;
    };
    let target_range = TextRange::empty(TextSize::new(0));
    Some(RangeInfo::new(
        include.range,
        vec![NavTarget {
            file_id: target_file_id,
            full_range: target_range,
            focus_range: Some(target_range),
            name: Some(path),
            kind: None,
            container_name: None,
            description: db.file_path(target_file_id).map(|path| path.to_string()),
        }],
    ))
}

fn handle_ctrl_flow_kw(
    sema: &Semantics<RootDb>,
    file_id: HirFileId,
    tp @ SyntaxTokenWithParent { .. }: SyntaxTokenWithParent,
) -> Option<Vec<NavTarget>> {
    let kind = tp.kind();

    match kind {
        _ if let Some(pair) = pair_token(tp) => {
            let tok = InFile::new(file_id, pair.either(|pair| pair, |_| tp));
            Some(vec![tok.to_nav(sema.db)?])
        }
        _ => None,
    }
}

pub(crate) fn token_precedence(kind: TokenKind) -> usize {
    match kind {
        _ if kind.name_like() => 4,
        _ if kind.is_pair_token() => 4,
        _ => 1,
    }
}
