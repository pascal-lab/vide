use hir::preproc::{IncludeDirective, IncludeTarget};

use crate::{
    RangeInfo,
    db::root_db::RootDb,
    markup::{Markup, inline_code},
    render,
};

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
