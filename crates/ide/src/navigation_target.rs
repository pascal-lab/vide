use base_db::intern::Lookup;
use hir::{
    container::{InContainer, InFile, InModule},
    db::HirDb,
    hir_def::{
        block::{BlockId, BlockLoc},
        declaration::Declaration,
        expr::declarator::{DeclId, DeclaratorParent},
        module::{ModuleId, instantiation::InstanceId, port::NonAnsiPortId},
        stmt::StmtId,
        subroutine::SubroutineId,
    },
    source_map::{IsNamedSrc, IsSrc},
};
use ide_db::root_db::RootDb;
use smol_str::SmolStr;
use syntax::{SyntaxTokenWithParent, has_text_range::HasTextRange};
use utils::{
    get::{Get, GetRef},
    line_index::TextRange,
};
use vfs::FileId;

use crate::{SymbolKind, definitions::DefinitionOrigin};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NavTarget {
    pub file_id: FileId,
    pub full_range: TextRange,
    pub focus_range: Option<TextRange>,

    pub name: Option<SmolStr>,
    pub kind: Option<SymbolKind>,
    pub container_name: Option<SmolStr>,
    // TODO: how to represent this?
    pub description: Option<String>,
}

impl NavTarget {
    pub fn focus_or_full_range(&self) -> TextRange {
        self.focus_range.unwrap_or(self.full_range)
    }
}

pub(crate) trait ToNav {
    fn to_nav(&self, db: &RootDb) -> NavTarget;
}

impl ToNav for DefinitionOrigin {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        match self {
            DefinitionOrigin::ModuleId(module_id) => module_id.to_nav(db),
            DefinitionOrigin::BlockId(block_id) => block_id.to_nav(db),
            DefinitionOrigin::SubroutineId(subroutine_id) => subroutine_id.to_nav(db),
            DefinitionOrigin::NonAnsiPort(nonansi_port_id) => nonansi_port_id.to_nav(db),
            DefinitionOrigin::Decl(decl_id) => decl_id.to_nav(db),
            DefinitionOrigin::Instance(instance_id) => instance_id.to_nav(db),
            DefinitionOrigin::Stmt(stmt_id) => stmt_id.to_nav(db),
        }
    }
}

impl ToNav for ModuleId {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InFile { value: local_module_id, file_id } = *self;
        let src = file_id.to_container_src_map(db).get(local_module_id);
        let name = self.to_container(db).name.clone();

        let file_id = file_id.file_id();
        build(file_id, src.name_range(), src.range(), name, SymbolKind::Module, None)
    }
}

impl ToNav for BlockId {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let BlockLoc { cont_id, src: InFile { value: src, file_id } } = self.lookup(db);
        let name = self.to_container(db).name.clone();
        let cont_name = cont_id.to_container(db).name().cloned();

        let file_id = file_id.file_id();
        build(file_id, src.name_range(), src.range(), name, SymbolKind::Block, cont_name)
    }
}

impl ToNav for SubroutineId {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let loc = self.lookup(db);
        let cont_name = loc.cont_id.to_container(db).name().cloned();
        let name = db.subroutine(*self).name.clone();

        let file_id = loc.src.file_id.file_id();
        build(file_id, None, loc.src.value.range(), name, SymbolKind::Fn, cont_name)
    }
}

impl ToNav for InModule<NonAnsiPortId> {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InModule { value: port_id, module_id } = *self;

        let file_id = module_id.file_id;
        let src = module_id.to_container_src_map(db).get(port_id);

        let module = db.module(module_id);
        let name = module.get(port_id).label.clone();
        let cont_name = module.name.clone();

        let file_id = file_id.file_id();
        build(file_id, src.name_range(), src.range(), name, SymbolKind::NonAnsiPortLabel, cont_name)
    }
}

impl ToNav for InContainer<DeclId> {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InContainer { value: decl_id, cont_id } = *self;

        let file_id = cont_id.file_id(db);
        let src = cont_id.to_container_src_map(db).get(decl_id);

        let cont = cont_id.to_container(db);
        let decl = cont.get(decl_id);

        let kind = match decl.parent {
            DeclaratorParent::PortDeclId(_) => SymbolKind::PortDecl,
            DeclaratorParent::DeclarationId(idx) => match cont.get(idx) {
                Declaration::DataDecl(_) => SymbolKind::DataDecl,
                Declaration::NetDecl(_) => SymbolKind::NetDecl,
                Declaration::ParamDecl(_) => SymbolKind::ParamDecl,
            },
            DeclaratorParent::StmtId(_) => SymbolKind::DataDecl,
        };

        let name = decl.name.clone();
        let cont_name = cont.name().cloned();

        build(file_id, src.name_range(), src.range(), name, kind, cont_name)
    }
}

impl ToNav for InModule<InstanceId> {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InModule { value: instance_id, module_id } = *self;

        let file_id = module_id.file_id();
        let src = module_id.to_container_src_map(db).get(instance_id);

        let module = module_id.to_container(db);
        let name = module.get(instance_id).name.clone();
        let cont_name = module.name.clone();

        build(file_id, src.name_range(), src.range(), name, SymbolKind::Instance, cont_name)
    }
}

impl ToNav for InContainer<StmtId> {
    fn to_nav(&self, db: &RootDb) -> NavTarget {
        let InContainer { value: stmt_id, cont_id } = *self;

        let file_id = cont_id.file_id(db);
        let src = cont_id.to_container_src_map(db).get(stmt_id);

        let cont = cont_id.to_container(db);
        let name = cont.get(stmt_id).label.clone();
        let cont_name = cont.name().cloned();

        build(file_id, src.name_range(), src.range(), name, SymbolKind::Stmt, cont_name)
    }
}

impl ToNav for InFile<SyntaxTokenWithParent<'_>> {
    fn to_nav(&self, _db: &RootDb) -> NavTarget {
        let InFile { value: SyntaxTokenWithParent { parent, tok }, file_id } = *self;
        NavTarget {
            file_id: file_id.file_id(),
            full_range: parent.text_range().unwrap(),
            focus_range: tok.text_range(),
            name: None,
            kind: None,
            container_name: None,
            description: None,
        }
    }
}

#[inline]
fn build(
    file_id: FileId,
    focus_range: Option<TextRange>,
    full_range: TextRange,
    name: Option<SmolStr>,
    kind: SymbolKind,
    container_name: Option<SmolStr>,
) -> NavTarget {
    let kind = Some(kind);
    NavTarget { file_id, full_range, focus_range, name, kind, container_name, description: None }
}
