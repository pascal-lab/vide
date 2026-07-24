use hir::{
    base_db::intern::Lookup,
    container::{InContainer, InFile, InModule, InSubroutine, SubroutineScope},
    db::HirDb,
    hir_def::{
        block::{BlockId, BlockLoc},
        declaration::Declaration,
        expr::declarator::{DeclId, DeclaratorParent},
        file::{config::ConfigDeclId, library::LibraryDeclId, udp::UdpDeclId},
        module::{
            ModuleId,
            generate::{GenerateBlockId, GenerateBlockLoc},
            instantiation::InstanceId,
            port::NonAnsiPortId,
        },
        stmt::StmtId,
        subroutine::SubroutinePortId,
        typedef::TypedefId,
    },
    source_map::{IsNamedSrc, IsSrc},
    symbol::DefOrigin,
};
use smol_str::SmolStr;
use syntax::{SyntaxTokenWithParent, has_text_range::HasTextRange};
use utils::{
    get::{Get, GetRef},
    line_index::TextRange,
};
use vfs::FileId;

use crate::{SymbolKind, db::root_db::RootDb};

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
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget>;
}

impl ToNav for DefOrigin {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let InFile { file_id, value: full_range } = self.range(db)?;
        let focus_range = self.name_range(db).map(|range| range.value);
        let name = self.name(db);
        let kind = self.kind(db).symbol_kind().into();
        let container_name = self.container_id(db).name(db);

        Some(build(file_id.file_id(), focus_range, full_range, name, kind, container_name))
    }
}

impl ToNav for ModuleId {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let InFile { value: local_module_id, file_id } = *self;
        let src = file_id.to_container_src_map(db).get(local_module_id)?;
        let name = self.to_container(db).name.clone();

        let file_id = file_id.file_id();
        Some(build(file_id, src.name_range(), src.range(), name, SymbolKind::Module, None))
    }
}

impl ToNav for InFile<ConfigDeclId> {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let InFile { value: config_id, file_id } = *self;
        let src = file_id.to_container_src_map(db).get(config_id)?;
        let name = file_id.to_container(db).get(config_id).name.clone();

        Some(build(
            file_id.file_id(),
            src.name_range(),
            src.range(),
            name,
            SymbolKind::Config,
            None,
        ))
    }
}

impl ToNav for InFile<LibraryDeclId> {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let InFile { value: library_id, file_id } = *self;
        let src = file_id.to_container_src_map(db).get(library_id)?;
        let name = file_id.to_container(db).get(library_id).name.clone();

        Some(build(
            file_id.file_id(),
            src.name_range(),
            src.range(),
            name,
            SymbolKind::Library,
            None,
        ))
    }
}

impl ToNav for InFile<UdpDeclId> {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let InFile { value: udp_id, file_id } = *self;
        let src = file_id.to_container_src_map(db).get(udp_id)?;
        let name = file_id.to_container(db).get(udp_id).name.clone();

        Some(build(
            file_id.file_id(),
            src.name_range(),
            src.range(),
            name,
            SymbolKind::Primitive,
            None,
        ))
    }
}

impl ToNav for BlockId {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let BlockLoc { cont_id, src: InFile { value: src, file_id } } = self.lookup(db);
        let name = self.to_container(db).name.clone();
        let cont_name = cont_id.data(db).name().cloned();

        let file_id = file_id.file_id();
        Some(build(file_id, src.name_range(), src.range(), name, SymbolKind::Block, cont_name))
    }
}

impl ToNav for GenerateBlockId {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let GenerateBlockLoc { cont_id, src: InFile { value: src, file_id } } = self.lookup(db);
        let name = self.to_container(db).name.clone();
        let cont_name = cont_id.data(db).name().cloned();

        Some(build(
            file_id.file_id(),
            src.name_range(),
            src.range(),
            name,
            SymbolKind::Generate,
            cont_name,
        ))
    }
}

impl ToNav for SubroutineScope {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        DefOrigin::new(db, *self).to_nav(db)
    }
}

impl ToNav for InSubroutine<SubroutinePortId> {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        DefOrigin::new(db, *self).to_nav(db)
    }
}

impl ToNav for InModule<NonAnsiPortId> {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let InModule { value: port_id, module_id } = *self;

        let file_id = module_id.file_id;
        let src = module_id.to_container_src_map(db).get(port_id)?;

        let module = db.module(module_id);
        let name = module.get(port_id).label.clone();
        let cont_name = module.name.clone();

        let file_id = file_id.file_id();
        Some(build(
            file_id,
            src.name_range(),
            src.range(),
            name,
            SymbolKind::NonAnsiPortLabel,
            cont_name,
        ))
    }
}

impl ToNav for InContainer<DeclId> {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let InContainer { value: decl_id, cont_id } = *self;

        let file_id = cont_id.file_id(db);
        let src = cont_id.source_map(db).get(decl_id)?;

        let cont = cont_id.data(db);
        let decl = cont.get(decl_id);

        let kind = match decl.parent {
            DeclaratorParent::PortDeclId(_) => SymbolKind::PortDecl,
            DeclaratorParent::DeclarationId(idx) => match cont.get(idx) {
                Declaration::DataDecl(_) => SymbolKind::DataDecl,
                Declaration::NetDecl(_) => SymbolKind::NetDecl,
                Declaration::ParamDecl(_) => SymbolKind::ParamDecl,
                Declaration::GenvarDecl(_) => SymbolKind::Genvar,
                Declaration::SpecparamDecl(_) => SymbolKind::Specparam,
            },
            DeclaratorParent::StmtId(_) => SymbolKind::DataDecl,
        };

        let name = decl.name.clone();
        let cont_name = cont.name().cloned();

        Some(build(file_id, src.name_range(), src.range(), name, kind, cont_name))
    }
}

impl ToNav for InContainer<TypedefId> {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let InContainer { value: typedef_id, cont_id } = *self;

        let file_id = cont_id.file_id(db);
        let src = cont_id.source_map(db).get(typedef_id)?;

        let cont = cont_id.data(db);
        let typedef = cont.get(typedef_id);
        let cont_name = cont.name().cloned();

        Some(build(
            file_id,
            src.name_range(),
            src.range(),
            typedef.name.clone(),
            SymbolKind::Typedef,
            cont_name,
        ))
    }
}

impl ToNav for InModule<InstanceId> {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let InModule { value: instance_id, module_id } = *self;

        let file_id = module_id.file_id();
        let src = module_id.to_container_src_map(db).get(instance_id)?;

        let module = module_id.to_container(db);
        let name = module.get(instance_id).name.clone();
        let cont_name = module.name.clone();

        Some(build(file_id, src.name_range(), src.range(), name, SymbolKind::Instance, cont_name))
    }
}

impl ToNav for InContainer<StmtId> {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let InContainer { value: stmt_id, cont_id } = *self;

        let file_id = cont_id.file_id(db);
        let src = cont_id.source_map(db).get(stmt_id)?;

        let cont = cont_id.data(db);
        let name = cont.get(stmt_id).label.clone();
        let cont_name = cont.name().cloned();

        Some(build(file_id, src.name_range(), src.range(), name, SymbolKind::Stmt, cont_name))
    }
}

impl ToNav for InFile<SyntaxTokenWithParent<'_>> {
    fn to_nav(&self, _db: &RootDb) -> Option<NavTarget> {
        let InFile { value: token, file_id } = *self;
        let full_range = token.parent.text_range()?;
        Some(NavTarget {
            file_id: file_id.file_id(),
            full_range,
            focus_range: token.text_range(),
            name: None,
            kind: None,
            container_name: None,
            description: None,
        })
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
