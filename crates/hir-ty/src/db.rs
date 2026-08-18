use std::ops::Deref;

use hir_def::db::HirDefDb;
#[salsa::db]
pub trait TyDb: HirDefDb {}

// See `HirDefDb` for why composed Salsa database objects use `Deref`.
impl Deref for dyn TyDb {
    type Target = dyn HirDefDb;

    fn deref(&self) -> &Self::Target {
        self
    }
}

impl dyn TyDb + '_ {}
