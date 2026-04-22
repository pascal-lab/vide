use base_db::source_db::SourceDb;
use hir::{
    container::{ContainerId, ContainerParent, InFile},
    display::HirDisplay,
    hir_def::{DEFAULT_NAME, literal::Literal},
    region_tree::RegionParent,
    semantics::Semantics,
};
use ide_db::root_db::RootDb;
use itertools::Itertools;
use syntax::{SVInt, SyntaxCursorExt, ast::AstNode, trivia::TriviaExt};
use utils::text_edit::TextSize;

use crate::{definitions::{Definition, DefinitionOrigin}, markup::Markup};

pub(crate) fn render_literal(literal: &Literal) -> Option<Markup> {
    let mut res = Markup::new();

    match literal {
        Literal::Int(svint) => {
            let width = svint.get_bit_width();
            let dec = render_svint(svint, 10);
            let mut info = format!("{dec} ({width} bits)");
            if let Some(ieee754) = render_svint_as_ieee754(svint) {
                info.push_str(&format!("\nieee754: {ieee754}"));
            }

            res.push_with_plain_fence(&info);
            res.new_section("Radix");

            let bin = render_svint(svint, 2);
            let oct = render_svint(svint, 8);
            let hex = render_svint(svint, 16);
            res.push_with_plain_fence(&format!("bin: {bin}\nhex: {hex}\noct: {oct}",));
        }
        Literal::Float(float) => {
            let num = f64::from(*float);
            let bits = float.to_bits();
            res.push_with_plain_fence(&format!("{num}\nbits: {bits:#x}"));
        }
        Literal::Time { val, unit } => {
            let num = f64::from(*val);
            res.push_with_plain_fence(&format!("{num} {unit}"));
        }
        Literal::Str(s) => {
            res.push_with_plain_fence(&format!("{s}"));
        }
        Literal::UnbasedUnsized(bit) => {
            res.push_with_plain_fence(&format!("{bit}"));
        }
    };

    Some(res)
}

fn render_svint(svint: &SVInt, base: usize) -> String {
    let mut s = svint.serialize(base);
    let mut len = s.len();
    let width = svint.get_bit_width();
    if base == 2 || base == 8 || base == 16 {
        let log = match base {
            2 => 1,
            8 => 3,
            16 => 4,
            _ => unreachable!(),
        };
        s.insert_str(0, &"0".repeat(width.div_ceil(log) - len));
        len += width.div_ceil(log) - len;
    }

    let interval = match base {
        2 => 4,
        8 => 3,
        10 => 3,
        16 => 4,
        _ => unreachable!("unexpected base: {base}"),
    };

    let mut result = String::with_capacity(len + len / interval + len / 4);

    for (i, c) in s.chars().enumerate() {
        if i > 0 {
            if base == 2 && (len - i).is_multiple_of(16) {
                result.push_str(" / ");
            } else if (len - i).is_multiple_of(interval) {
                result.push(' ');
            }
        }
        result.push(c);
    }

    result
}

fn render_svint_as_ieee754(svint: &SVInt) -> Option<String> {
    let width = svint.get_bit_width();

    if (width != 32 && width != 64) || svint.has_unknown() {
        return None;
    }

    let word = svint.get_single_word().unwrap();
    if width == 32 {
        let f = f32::from_bits(word as u32);
        Some(format!("{:?}", f))
    } else {
        let f = f64::from_bits(word);
        Some(format!("{:?}", f))
    }
}

pub(crate) fn render_definition(sema: &Semantics<RootDb>, def: Definition) -> Markup {
    def.def_origins().into_iter().fold(Markup::new(), |mut res, origin| {
        res.merge(render_def_origin(sema, &origin));
        res
    })
}

fn render_def_origin(sema: &Semantics<RootDb>, origin: &DefinitionOrigin) -> Markup {
    let mut res = Markup::new();

    if let Some(signature) = render_signature(sema, origin) {
        res.push_with_code_fence(&signature);
    }

    res.merge(render_containers(sema, origin));

    if let Some(markup) = render_side_comments(sema, origin) {
        if !res.is_empty() {
            res.newline();
        }
        res.merge(markup);
    }

    res
}

fn render_signature(sema: &Semantics<RootDb>, origin: &DefinitionOrigin) -> Option<String> {
    let db = sema.db;
    match origin {
        DefinitionOrigin::Typedef(typedef) => typedef.display_signature(db).ok(),
        _ => None,
    }
}

fn render_side_comments(sema: &Semantics<'_, RootDb>, origin: &DefinitionOrigin) -> Option<Markup> {
    let db = sema.db;
    let InFile { value: range, file_id } = origin.range(db);
    let end = range.end();

    let text = db.file_text(file_id.file_id());
    let text = &text[end.into()..];
    let relative_start = 'out: {
        for ((pos, c), (_, c1)) in text.char_indices().tuple_windows() {
            match c {
                '\n' => return None,
                '/' if matches!(c1, '/' | '*') => break 'out pos as u32,
                _ => {}
            }
        }
        return None;
    };

    let root = sema.parse(file_id.file_id());
    let mut cursor = root.syntax().walk();
    cursor.goto_first_tok_after_or_last(end + TextSize::new(relative_start));
    cursor
        .to_token()?
        .trivias()
        .find_map(|t| t.as_comment().map(|c| c.to_string()))
        .map(|comment| comment.into())
}

fn render_containers(sema: &Semantics<RootDb>, origin: &DefinitionOrigin) -> Markup {
    // elaboration?
    let db = sema.db;
    let InFile { value: range, .. } = origin.range(db);
    let cont_id = origin.container_id(db);

    let mut containers = Vec::new();

    for cont_id in ContainerParent::start_from(db, cont_id) {
        let src_map = cont_id.to_container_src_map(db);

        if let Some(region_tree) = src_map.region_tree()
            && let Some(node) = region_tree.find(range.start())
        {
            for region in RegionParent::start_from(region_tree, node) {
                containers.push(format!("({})", region.name()));
            }
        }

        if !matches!(cont_id, ContainerId::HirFileId(_)) {
            if let Some(name) = cont_id.to_container(db).name() {
                containers.push(name.to_string());
            } else {
                containers.push(DEFAULT_NAME.to_string());
            }
        }
    }

    let mut ans = Markup::new();
    ans.push_with_code_fence(&containers.into_iter().rev().join(" > "));
    ans
}
