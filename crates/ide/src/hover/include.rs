use hir::{
    base_db::source_db::SourceDb,
    preproc::{IncludeDirective, IncludeTarget, include_directives_at},
};
use utils::line_index::TextSize;
use vfs::FileId;

use crate::{RangeInfo, db::root_db::RootDb, markup::Markup};

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
    markup.print("Include");
    for include in includes {
        markup.newline();
        match include.target {
            IncludeTarget::Literal { path, resolved_file } => {
                markup.push_with_backticks(path.as_str());
                if let Some(target_file_id) = resolved_file
                    && let Some(path) = db.file_path(target_file_id)
                {
                    markup.newline();
                    markup.print(&path.to_string());
                }
            }
            IncludeTarget::Token { raw } => {
                markup.push_with_backticks(raw.as_str());
            }
        }
    }
    Some(RangeInfo::new(range, markup))
}
