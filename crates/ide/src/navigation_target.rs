use hir::{
    base_db::intern::Lookup,
    container::{InContainer, InFile, InModule, InSubroutine, ScopeId},
    db::HirDb,
    file::HirFileId,
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
        subroutine::{LocalSubroutineId, SubroutinePortId},
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
        let container_name = match self.container_id(db) {
            ScopeId::File(_) => None,
            cont_id => cont_id.to_container(db).name().cloned(),
        };

        let (file_id, focus_range, full_range) =
            nav_location(db, file_id, focus_range, full_range)?;
        Some(build(file_id, focus_range, full_range, name, kind, container_name))
    }
}

impl ToNav for ModuleId {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let InFile { value: local_module_id, file_id } = *self;
        let src = file_id.to_container_src_map(db).get(local_module_id)?;
        let name = self.to_container(db).name.clone();

        let (file_id, focus_range, full_range) =
            nav_location(db, file_id, src.name_range(), src.range())?;
        Some(build(file_id, focus_range, full_range, name, SymbolKind::Module, None))
    }
}

impl ToNav for InFile<ConfigDeclId> {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let InFile { value: config_id, file_id } = *self;
        let src = file_id.to_container_src_map(db).get(config_id)?;
        let name = file_id.to_container(db).get(config_id).name.clone();

        let (file_id, focus_range, full_range) =
            nav_location(db, file_id, src.name_range(), src.range())?;
        Some(build(file_id, focus_range, full_range, name, SymbolKind::Config, None))
    }
}

impl ToNav for InFile<LibraryDeclId> {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let InFile { value: library_id, file_id } = *self;
        let src = file_id.to_container_src_map(db).get(library_id)?;
        let name = file_id.to_container(db).get(library_id).name.clone();

        let (file_id, focus_range, full_range) =
            nav_location(db, file_id, src.name_range(), src.range())?;
        Some(build(file_id, focus_range, full_range, name, SymbolKind::Library, None))
    }
}

impl ToNav for InFile<UdpDeclId> {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let InFile { value: udp_id, file_id } = *self;
        let src = file_id.to_container_src_map(db).get(udp_id)?;
        let name = file_id.to_container(db).get(udp_id).name.clone();

        let (file_id, focus_range, full_range) =
            nav_location(db, file_id, src.name_range(), src.range())?;
        Some(build(file_id, focus_range, full_range, name, SymbolKind::Primitive, None))
    }
}

impl ToNav for BlockId {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let BlockLoc { cont_id, src: InFile { value: src, file_id } } = self.lookup(db);
        let name = self.to_container(db).name.clone();
        let cont_name = cont_id.to_container(db).name().cloned();

        let (file_id, focus_range, full_range) =
            nav_location(db, file_id, src.name_range(), src.range())?;
        Some(build(file_id, focus_range, full_range, name, SymbolKind::Block, cont_name))
    }
}

impl ToNav for GenerateBlockId {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let GenerateBlockLoc { cont_id, src: InFile { value: src, file_id } } = self.lookup(db);
        let name = self.to_container(db).name.clone();
        let cont_name = cont_id.to_container(db).name().cloned();

        let (file_id, focus_range, full_range) =
            nav_location(db, file_id, src.name_range(), src.range())?;
        Some(build(file_id, focus_range, full_range, name, SymbolKind::Generate, cont_name))
    }
}

impl ToNav for InContainer<LocalSubroutineId> {
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

        let (file_id, focus_range, full_range) =
            nav_location(db, file_id, src.name_range(), src.range())?;
        Some(build(file_id, focus_range, full_range, name, SymbolKind::NonAnsiPortLabel, cont_name))
    }
}

impl ToNav for InContainer<DeclId> {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let InContainer { value: decl_id, cont_id } = *self;

        let file_id = cont_id.file_id(db);
        let src = cont_id.to_container_src_map(db).get(decl_id)?;

        let cont = cont_id.to_container(db);
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

        let (file_id, focus_range, full_range) =
            nav_location(db, file_id, src.name_range(), src.range())?;
        Some(build(file_id, focus_range, full_range, name, kind, cont_name))
    }
}

impl ToNav for InContainer<TypedefId> {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let InContainer { value: typedef_id, cont_id } = *self;

        let file_id = cont_id.file_id(db);
        let src = cont_id.to_container_src_map(db).get(typedef_id)?;

        let cont = cont_id.to_container(db);
        let typedef = cont.get(typedef_id);
        let cont_name = cont.name().cloned();

        let (file_id, focus_range, full_range) =
            nav_location(db, file_id, src.name_range(), src.range())?;
        Some(build(
            file_id,
            focus_range,
            full_range,
            typedef.name.clone(),
            SymbolKind::Typedef,
            cont_name,
        ))
    }
}

impl ToNav for InModule<InstanceId> {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let InModule { value: instance_id, module_id } = *self;

        let file_id = module_id.file_id;
        let src = module_id.to_container_src_map(db).get(instance_id)?;

        let module = module_id.to_container(db);
        let name = module.get(instance_id).name.clone();
        let cont_name = module.name.clone();

        let (file_id, focus_range, full_range) =
            nav_location(db, file_id, src.name_range(), src.range())?;
        Some(build(file_id, focus_range, full_range, name, SymbolKind::Instance, cont_name))
    }
}

impl ToNav for InContainer<StmtId> {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let InContainer { value: stmt_id, cont_id } = *self;

        let file_id = cont_id.file_id(db);
        let src = cont_id.to_container_src_map(db).get(stmt_id)?;

        let cont = cont_id.to_container(db);
        let name = cont.get(stmt_id).label.clone();
        let cont_name = cont.name().cloned();

        let (file_id, focus_range, full_range) =
            nav_location(db, file_id, src.name_range(), src.range())?;
        Some(build(file_id, focus_range, full_range, name, SymbolKind::Stmt, cont_name))
    }
}

impl ToNav for InFile<SyntaxTokenWithParent<'_>> {
    fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
        let InFile { value: token, file_id } = *self;
        let full_range = token.parent.text_range()?;
        let (file_id, focus_range, full_range) =
            nav_location(db, file_id, token.text_range(), full_range)?;
        Some(NavTarget {
            file_id,
            full_range,
            focus_range,
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

/// Resolves a HIR file location to a user-facing source file and range.
///
/// For real files the location is returned as-is. For macro expansions the
/// location is mapped to the macro invocation site, since the expanded text is
/// not a file the user can open: both the file and the range point at the
/// macro call. Returns `None` when a macro expansion's call site cannot be
/// resolved.
pub(crate) fn nav_location(
    db: &dyn HirDb,
    file_id: HirFileId,
    name_range: Option<TextRange>,
    full_range: TextRange,
) -> Option<(FileId, Option<TextRange>, TextRange)> {
    match file_id {
        HirFileId::File(file_id) => Some((file_id, name_range, full_range)),
        HirFileId::Macro(macro_file) => {
            let expansion = hir::hir_def::macro_file::macro_file_expansion(db, macro_file)?;
            Some((expansion.call_file_id, Some(expansion.call_range), expansion.call_range))
        }
    }
}
