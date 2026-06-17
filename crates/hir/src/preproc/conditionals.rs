use super::*;

pub fn inactive_branches(
    db: &dyn SourceRootDb,
    file_id: FileId,
) -> PreprocResult<Vec<InactiveBranch>> {
    let mut branches = UniqVec::<InactiveBranch, InactiveBranchKey>::default();
    let mut first_error = None;
    let contexts = source_preproc_single_query_contexts(db, file_id);

    for model_file_id in contexts.model_file_ids.iter().copied() {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };

        for source_range in mapped.model.inactive_ranges() {
            let (source, range) = match map_mapped_source_range(mapped, *source_range) {
                Ok(mapped_range) => mapped_range,
                Err(error) => {
                    record_first_error(&mut first_error, error);
                    continue;
                }
            };
            let branch_file_id = match require_file_backed_source(&source) {
                Ok(file_id) => file_id,
                Err(error) => {
                    record_first_error(&mut first_error, error);
                    continue;
                }
            };
            if branch_file_id == file_id {
                let branch = InactiveBranch { file_id: branch_file_id, range };
                branches.push_keyed(branch, InactiveBranchKey::from_branch);
            }
        }
    }

    if branches.is_empty()
        && let Err(error) = finish_empty_single_query(&contexts, first_error)
    {
        return Err(error);
    }

    Ok(branches.into_vec())
}
