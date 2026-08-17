use preproc_expand::macro_file::SourceEmittedTokenId;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use vfs::FileId;

use super::{FileNameIndex, NameOccurrence};
use crate::db::workspace_symbol_index_db::WorkspaceSymbolIndexDb;

pub(super) fn collect_file(db: &dyn WorkspaceSymbolIndexDb, file_id: FileId) -> FileNameIndex {
    let shard = db.file_decl_shard(file_id);
    let mut occurrences: FxHashMap<SmolStr, Vec<NameOccurrence>> = FxHashMap::default();
    for mention in shard.mentions.iter() {
        occurrences.entry(mention.name.clone()).or_default().push(NameOccurrence {
            range: mention.range,
            kind: mention.kind,
            emitted: mention
                .emitted
                .and_then(|index| usize::try_from(index).ok())
                .map(SourceEmittedTokenId::new),
        });
    }
    FileNameIndex {
        occurrences: occurrences
            .into_iter()
            .map(|(name, entries)| (name, entries.into_boxed_slice()))
            .collect(),
    }
}
