pub trait Intern {
    type Database<'db>: ?Sized;
    type ID;
    fn intern(self, db: &Self::Database<'_>) -> Self::ID;
}

pub trait Lookup {
    type Database<'db>: ?Sized;
    type Data;
    fn lookup(&self, db: &Self::Database<'_>) -> Self::Data;
}

#[macro_export]
macro_rules! impl_intern_key {
    ($name:ident) => {
        impl $crate::base_db::salsa::InternKey for $name {
            fn from_intern_id(v: $crate::base_db::salsa::InternId) -> Self {
                $name(v)
            }

            fn as_intern_id(&self) -> $crate::base_db::salsa::InternId {
                self.0
            }
        }
    };
}

#[macro_export]
macro_rules! impl_intern_lookup {
    ($db:ident, $id:ident, $loc:ident, $intern:ident, $lookup:ident) => {
        impl $crate::base_db::intern::Intern for $loc {
            type Database<'db> = dyn $db + 'db;
            type ID = $id;

            fn intern(self, db: &Self::Database<'_>) -> $id {
                db.$intern(self)
            }
        }

        impl $crate::base_db::intern::Lookup for $id {
            type Data = $loc;
            type Database<'db> = dyn $db + 'db;

            fn lookup(&self, db: &Self::Database<'_>) -> $loc {
                db.$lookup(*self)
            }
        }
    };
}
