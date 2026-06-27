use hir::base_db::{
    project::ProjectConfig,
    source_db::{preproc_virtual_predefines_path, preproc_virtual_predefines_text},
};
use utils::lines::LineEnding;
use vfs::{Vfs, loader::LoadResult};

pub(crate) fn materialize_preproc_virtual_files(project_config: &ProjectConfig, vfs: &mut Vfs) {
    for profile_id in project_config.profile_ids() {
        let preprocess = project_config.preprocess_for_profile(Some(profile_id));
        if preprocess.predefines.is_empty() {
            continue;
        }

        let path = preproc_virtual_predefines_path(Some(profile_id));
        let text = preproc_virtual_predefines_text(&preprocess.predefines);
        vfs.set_file_contents(&path, LoadResult::Loaded(text, LineEnding::Unix));
    }
}
