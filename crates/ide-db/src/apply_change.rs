use base_db::{
    change::Change,
    salsa::{Database, Durability},
    source_root::SourceRootId,
};
use itertools::{Either, Itertools};
use rustc_hash::FxHashSet;

use crate::root_db::RootDb;

impl RootDb {
    pub fn request_cancellation(&mut self) {
        // `synthetic_write` triggers cancellation, it will block until snapshots
        // are dropped, which might trigger deadlock.
        self.salsa_runtime_mut().synthetic_write(Durability::LOW);
    }

    pub fn apply_change(&mut self, change: Change) {
        self.request_cancellation();
        tracing::trace!("apply_change {:?}", change);

        if let Some(roots) = &change.roots {
            let (_lib_roots, _local_roots): (FxHashSet<_>, FxHashSet<_>) =
                roots.iter().enumerate().partition_map(|(idx, root)| {
                    let source_root_id = SourceRootId(idx as u32);
                    if root.is_lib {
                        Either::Left(source_root_id)
                    } else {
                        Either::Right(source_root_id)
                    }
                });

            // TODO: set roots
        }
        change.apply(self);
    }
}
