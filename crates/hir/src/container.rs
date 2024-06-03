use base_db::intern::Lookup;
use utils::impl_from;

use crate::{
    db::HirDb,
    file::HirFileId,
    hir_def::{
        block::{BlockId, BlockLoc},
        ModuleId,
    },
};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ContainerId {
    // [`HirFileId`] is a special container.
    HirFileId(HirFileId),
    ModuleId(ModuleId),
    BlockId(BlockId),
}

impl_from!(HirFileId, ModuleId, BlockId for ContainerId);

macro_rules! impl_contained {
    ($($container:ident[$field:ident: $id:ident]),*) => {
        $(
            #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
            pub struct $container<T> {
                pub value: T,
                pub $field: $id,
            }

            impl<T> From<$container<T>> for InContainer<T> {
                fn from(container: $container<T>) -> InContainer<T> {
                    InContainer::new(ContainerId::$id(container.$field), container.value)
                }
            }

            impl<T> $container<T> {
                pub fn new($field: $id, value: T) -> $container<T> {
                    $container { $field, value }
                }

                pub fn with_value<U>(self, value: U) -> $container<U> {
                    $container::new(self.$field, value)
                }
            }
        )*
    };
}

impl_contained!(
    InModule[module_id: ModuleId],
    InBlock[block_id: BlockId],
    InFile[file_id: HirFileId]
);

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct InContainer<T> {
    pub value: T,
    pub container_id: ContainerId,
}

impl<T> InContainer<T> {
    fn new(container_id: ContainerId, value: T) -> InContainer<T> {
        InContainer { value, container_id }
    }

    fn with_value<U>(self, value: U) -> InContainer<U> {
        InContainer::new(self.container_id, value)
    }
}

/// Parents of a scope.
pub struct ContainerParent<'db> {
    db: &'db dyn HirDb,
    container_id: Option<ContainerId>,
}

impl<'db> ContainerParent<'db> {
    pub fn new(db: &'db dyn HirDb, container_id: ContainerId) -> ContainerParent {
        ContainerParent { db, container_id: Some(container_id) }
    }
}

impl<'db> Iterator for ContainerParent<'db> {
    type Item = ContainerId;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.container_id;
        match self.container_id {
            Some(ContainerId::ModuleId(module_id)) => {
                self.container_id = Some(ContainerId::HirFileId(module_id.file_id));
            }
            Some(ContainerId::BlockId(block_id)) => {
                let BlockLoc { container_id, .. } = block_id.lookup(self.db);
                self.container_id = Some(container_id);
            }
            _ => {
                self.container_id = None;
            }
        }
        next
    }
}
