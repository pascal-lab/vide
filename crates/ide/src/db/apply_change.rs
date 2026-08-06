use base_db::{
    change::Change,
    salsa::{Database, Durability},
};

use crate::db::root_db::RootDb;

impl RootDb {
    pub fn request_cancellation(&mut self) {
        // `synthetic_write` triggers cancellation, it will block until snapshots
        // are dropped, which might trigger deadlock.
        self.synthetic_write(Durability::LOW);
    }

    pub fn apply_change(&mut self, change: Change) {
        self.request_cancellation();
        tracing::trace!("apply_change {:?}", change);
        change.apply(self);
    }
}
