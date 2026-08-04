use std::collections::BTreeMap;

use smol_str::SmolStr;

use super::types::*;

macro_rules! source_table_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(usize);

        impl $name {
            pub fn new(raw: usize) -> Self {
                Self(raw)
            }

            pub fn raw(self) -> usize {
                self.0
            }
        }
    };
}

macro_rules! source_table {
    ($table:ident, $field:ident, $id:ident, $item:ty) => {
        #[derive(Debug, Clone, PartialEq, Eq, Default)]
        pub struct $table {
            $field: Vec<$item>,
        }

        impl $table {
            pub fn get(&self, id: $id) -> Option<&$item> {
                self.$field.get(id.raw())
            }

            pub fn iter(&self) -> std::slice::Iter<'_, $item> {
                self.$field.iter()
            }

            pub fn len(&self) -> usize {
                self.$field.len()
            }

            pub fn is_empty(&self) -> bool {
                self.$field.is_empty()
            }

            pub(in crate::source::tables) fn push(&mut self, item: $item) {
                self.$field.push(item);
            }
        }
    };

    ($table:ident, $field:ident, $id:ident, $item:ty,mutable) => {
        source_table!($table, $field, $id, $item);

        impl $table {
            pub(in crate::source::tables) fn get_mut(&mut self, id: $id) -> Option<&mut $item> {
                self.$field.get_mut(id.raw())
            }
        }
    };
}

source_table_id!(SourceMacroDefinitionId);
source_table_id!(SourceMacroReferenceId);
source_table_id!(SourceIncludeDirectiveId);
source_table_id!(SourceMacroStateId);
source_table_id!(SourceMacroCallId);

mod unavailable;
pub use unavailable::*;

mod items;
pub use items::*;

mod storage;
pub use storage::*;

mod builder;
pub(in crate::source) use builder::SourcePreprocModelBuilder;
