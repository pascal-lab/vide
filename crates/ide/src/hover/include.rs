use hir::preproc::{IncludeDirective, IncludeTarget, include_directives_at};
use utils::line_index::TextSize;
use vfs::FileId;

use crate::{
    RangeInfo,
    db::root_db::RootDb,
    markup::{Markup, inline_code},
    render,
};

pub(super) fn dispatch_include_hover_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<Vec<IncludeDirective>> {
    let includes = include_directives_at(db, file_id, offset).ok()?;
    (!includes.is_empty()).then_some(includes)
}

pub(super) fn render_include_hover(
    db: &RootDb,
    includes: Vec<IncludeDirective>,
) -> Option<RangeInfo<Markup>> {
    let range = includes.first()?.range;
    let mut markup = Markup::new();
    for (idx, include) in includes.into_iter().enumerate() {
        if idx > 0 {
            markup.horizontal_line();
        }
        match include.target {
            IncludeTarget::Literal { path, resolved_file } => {
                markup.title(&format!("Include {}", inline_code(path.as_str())));
                markup.push_with_code_fence(&format!("`include \"{}\"", path.as_str()));
                let resolved = resolved_file
                    .and_then(|target_file_id| {
                        render::source_file_link(db, target_file_id, include.file_id)
                    })
                    .unwrap_or_else(|| "unresolved".to_string());
                markup.metadata_line(&format!("resolves to {resolved}"));
            }
            IncludeTarget::Token { raw } => {
                markup.title(&format!("Include {}", inline_code(raw.as_str())));
                markup.push_with_code_fence(&format!("`include {}", raw.as_str()));
            }
        }
    }
    Some(RangeInfo::new(range, markup))
}
