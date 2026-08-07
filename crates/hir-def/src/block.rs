use utils::define_enum_deriving_from;

use crate::{aggregate::StructId, declaration::DeclarationId, stmt::StmtId, typedef::TypedefId};
define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone)]
    pub enum BlockItem {
        DeclarationId,
        TypedefId,
        StructId,
        StmtId,
    }
}
