use super::*;

pub fn inactive_branches(
    db: &dyn PreprocDb,
    file_id: FileId,
) -> PreprocResult<Vec<InactiveBranch>> {
    let mut branches = UniqVec::<InactiveBranch, InactiveBranchKey>::default();
    let mut query = ContextQuery::new(db, file_id);
    query.for_each_model(db, |_model_file_id, mapped| {
        for source_range in mapped.model.inactive_ranges() {
            let (source, range) = map_source_mapping_range(mapped, *source_range)?;
            let branch_file_id = require_file_backed_source(&source)?;
            if branch_file_id == file_id {
                let branch = InactiveBranch { file_id: branch_file_id, range };
                branches.push_keyed(branch, InactiveBranchKey::from_branch);
            }
        }
        Ok(())
    });
    query.finish()?;

    Ok(branches.into_vec())
}
