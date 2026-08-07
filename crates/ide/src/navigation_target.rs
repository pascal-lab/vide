use hir_def::{
    container::{InContainer, InFile, InModule, InSubroutine, SubroutineScope},
    expr::declarator::DeclId,
    file::{config::ConfigDeclId, library::LibraryDeclId, udp::UdpDeclId},
    has_source::HasSource,
    module::{ModuleId, generate::GenerateBlockId, instantiation::InstanceId, port::NonAnsiPortId},
    owner::OwnerId,
    stmt::StmtId,
    subroutine::SubroutinePortId,
    symbol::{DefOrigin, DefOriginLoc},
    typedef::TypedefId,
};
use hir_ty::db::TyDb;
use preproc_expand::file::HirFileId;
use smol_str::SmolStr;
use syntax::{SyntaxTokenWithParent, has_text_range::HasTextRange};
use utils::line_index::TextRange;
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
        let InFile { file_id, value: source } = self.source(db)?;
        let full_range = source.full_range();
        let focus_range = source.focus_range();
        let name = self.name(db);
        let kind = self.kind(db).symbol_kind().into();
        let container_name = self.container_id(db).name(db);

        let (file_id, focus_range, full_range) =
            nav_location(db, file_id, focus_range, full_range)?;
        Some(build(file_id, focus_range, full_range, name, kind, container_name))
    }
}

macro_rules! impl_to_nav_via_origin {
    ($($ty:ty),* $(,)?) => {
        $(
            impl ToNav for $ty {
                fn to_nav(&self, db: &RootDb) -> Option<NavTarget> {
                    DefOrigin::new(db, DefOriginLoc::from(self.clone())).to_nav(db)
                }
            }
        )*
    };
}

impl_to_nav_via_origin!(
    ModuleId,
    InFile<ConfigDeclId>,
    InFile<LibraryDeclId>,
    InFile<UdpDeclId>,
    OwnerId,
    GenerateBlockId,
    SubroutineScope,
    InSubroutine<SubroutinePortId>,
    InModule<NonAnsiPortId>,
    InContainer<DeclId>,
    InContainer<TypedefId>,
    InModule<InstanceId>,
    InContainer<StmtId>,
);

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
    db: &dyn TyDb,
    file_id: HirFileId,
    name_range: Option<TextRange>,
    full_range: TextRange,
) -> Option<(FileId, Option<TextRange>, TextRange)> {
    match file_id {
        HirFileId::File(file_id) => Some((file_id, name_range, full_range)),
        HirFileId::Macro(macro_file) => {
            let call_site = preproc_expand::macro_file::macro_file_call_site(db, macro_file)?;
            Some((call_site.call_file_id, Some(call_site.call_range), call_site.call_range))
        }
    }
}
