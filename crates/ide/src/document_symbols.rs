use std::iter::Peekable;

use hir::{
    db::HirDb,
    file::HirFileId,
    hir_def::{
        DEFAULT_NAME,
        block::{BlockId, BlockInfo, BlockItem, BlockSrc, LocalBlockId},
        declaration::{Declaration, DeclarationId, DeclarationSrc},
        expr::declarator::{DeclId, Declarator, DeclaratorSrc, DeclsRange},
        file::FileItem,
        module::{ModuleId, ModuleItem, ModuleSrc, port::Ports},
        stmt::{CaseItem, ForInit, Stmt, StmtId, StmtKind, StmtSrc},
        typedef::{Typedef, TypedefId, TypedefSrc},
    },
    region_tree::{RegionNode, RegionTreeIterator},
    source_map::{IsNamedSrc, IsSrc},
};
use ide_db::root_db::RootDb;
use la_arena::Idx;
use smol_str::SmolStr;
use syntax::WalkEvent;
use utils::{
    get::{Get, GetRef},
    line_index::TextRange,
};
use vfs::FileId;

use crate::SymbolKind;

#[derive(Debug, Clone)]
pub struct DocumentSymbol {
    pub name: String,
    pub focus_range: TextRange,
    pub full_range: TextRange,
    pub kind: SymbolKind,
    pub detail: Option<String>,
    pub container_name: Option<String>,
    pub children: Vec<DocumentSymbol>,
}

#[derive(Debug, Clone)]
struct SymbolCollecter {
    res: Vec<DocumentSymbol>,
    stack: Vec<DocumentSymbol>,
}

impl SymbolCollecter {
    pub fn new(len: usize) -> Self {
        Self { res: Vec::with_capacity(len), stack: Vec::with_capacity(len) }
    }

    pub fn push_symbol(&mut self, name: &Option<SmolStr>, src: impl IsNamedSrc) {
        let container_name = self.stack.last().map(|sym| sym.name.to_owned());
        let sym = DocumentSymbol {
            name: name.as_ref().unwrap_or(&DEFAULT_NAME).to_string(),
            focus_range: src.name_or_full_range(),
            full_range: src.range(),
            kind: SymbolKind::from_syntax_kind(src.kind()),
            detail: None,
            container_name,
            children: Vec::new(),
        };
        self.stack.push(sym);
    }

    pub fn push_symbol_with_kind(
        &mut self,
        name: &Option<SmolStr>,
        src: impl IsNamedSrc,
        kind: SymbolKind,
    ) {
        self.push_symbol(name, src);
        self.stack.last_mut().unwrap().kind = kind;
    }

    pub fn push_symbol_with_children(
        &mut self,
        name: &Option<SmolStr>,
        src: impl IsNamedSrc,
        len: usize,
    ) {
        self.push_symbol(name, src);

        if let Some(parent) = self.stack.last_mut() {
            parent.children.reserve(len);
        } else {
            self.res.reserve(len);
        }
    }

    pub fn push_region(&mut self, region: &RegionNode) {
        let container_name = self.stack.last().map(|sym| sym.name.to_owned());
        let sym = DocumentSymbol {
            name: region.name().to_string(),
            focus_range: region.focus_range(),
            full_range: region.range,
            kind: SymbolKind::Region,
            detail: None,
            container_name,
            children: Vec::new(),
        };
        self.stack.push(sym);
    }

    #[inline]
    pub fn pop(&mut self) {
        let mut sym = self.stack.pop().unwrap();

        if (sym.kind == SymbolKind::Block
            || sym.kind == SymbolKind::Stmt
            || sym.kind == SymbolKind::Region)
            && sym.name == DEFAULT_NAME
            && sym.children.is_empty()
        {
            return;
        }

        sym.children.shrink_to_fit();

        if let Some(parent) = self.stack.last_mut() {
            parent.children.push(sym);
        } else {
            self.res.push(sym);
        }
    }

    pub fn finish(self) -> Vec<DocumentSymbol> {
        assert!(self.stack.is_empty(), "{:?}", &self.stack);
        self.res
    }
}

trait AddRegionSymbol {
    fn add_region_symbol(&mut self, node_range: TextRange, collector: &mut SymbolCollecter);
    fn finish_all(&mut self, collector: &mut SymbolCollecter);
}

impl AddRegionSymbol for Peekable<RegionTreeIterator<'_>> {
    #[inline]
    fn add_region_symbol<'a>(&mut self, node_range: TextRange, collector: &mut SymbolCollecter) {
        loop {
            match self.peek() {
                Some(WalkEvent::Enter(region)) if region.range.start() <= node_range.start() => {
                    collector.push_region(region);
                }
                Some(WalkEvent::Leave(region)) if region.range.end() <= node_range.start() => {
                    collector.pop();
                }
                _ => break,
            }
            self.next().unwrap();
        }
    }

    #[inline]
    fn finish_all(&mut self, collector: &mut SymbolCollecter) {
        for event in self {
            match event {
                WalkEvent::Enter(region) => collector.push_region(region),
                WalkEvent::Leave(_) => collector.pop(),
            }
        }
    }
}

// TODO: add ty info in detail
pub(crate) fn document_symbols(db: &RootDb, file_id: FileId) -> Vec<DocumentSymbol> {
    let file_id = HirFileId(file_id);
    let (file, src_map) = db.hir_file_with_source_map(file_id);
    let (file, src_map) = (file.as_ref(), src_map.as_ref());
    let mut regions = src_map.region_tree.walk().peekable();

    let mut collector = SymbolCollecter::new(
        src_map.items.len() + src_map.region_tree.roots.len() + file.decls.len(),
    );

    for &item in src_map.items.iter() {
        regions.add_region_symbol(src_map.item_to_ptr(&item).range(), &mut collector);

        match item {
            FileItem::LocalModuleId(idx) => {
                let module_id = ModuleId::new(file_id, idx);
                let module_src = src_map.get(idx);
                collect_module_items(db, module_id, module_src, &mut collector);
            }
            FileItem::ProcId(proc_id) => {
                let proc = file.get(proc_id);
                let stmt_id = proc.stmt;
                build_stmt(db, &mut collector, stmt_id, file, src_map);
            }
            FileItem::DeclarationId(declaration_id) => {
                build_declaration(&mut collector, declaration_id, file, src_map);
            }
            FileItem::TypedefId(typedef_id) => build_typedef(&mut collector, typedef_id, file, src_map),
            FileItem::StructId(_) | FileItem::SubroutineId(_) => {
                // TODO: implement document symbols for these items
            }
        }
    }

    regions.finish_all(&mut collector);
    collector.finish()
}

fn collect_module_items(
    db: &RootDb,
    module_id: ModuleId,
    module_src: ModuleSrc,
    collector: &mut SymbolCollecter,
) {
    let (module, src_map) = db.module_with_source_map(module_id);
    let (module, src_map) = (module.as_ref(), src_map.as_ref());
    let mut regions = src_map.region_tree.walk().peekable();

    collector.push_symbol_with_children(
        &module.name,
        module_src,
        src_map.items.len() + module.decls.len() + module.stmts.len(),
    );

    if let Some(params) = &module.param_ports {
        for decl_id in params.clone() {
            let src = src_map.get(decl_id);
            regions.add_region_symbol(src.range(), collector);
            build_decl(collector, decl_id, SymbolKind::ParamDecl, module, src_map);
        }
    }

    match &module.ports {
        Ports::NonAnsi { ports, .. } => {
            for (port_id, port) in ports.iter() {
                let src = src_map.get(port_id);
                regions.add_region_symbol(src.range(), collector);
                collector.push_symbol(&port.label, src);
                collector.pop();
            }
        }
        Ports::Ansi(port_decls) => {
            for (port_id, port_decl) in port_decls.iter() {
                let src = src_map.get(port_id);
                regions.add_region_symbol(src.range(), collector);
                build_decls(collector, &port_decl.decls, SymbolKind::PortDecl, module, src_map);
            }
        }
    }

    for item in src_map.items.iter() {
        regions.add_region_symbol(src_map.item_to_ptr(item).range(), collector);
        match *item {
            ModuleItem::DeclarationId(declaration_id) => {
                build_declaration(collector, declaration_id, module, src_map)
            }
            ModuleItem::InstantiationId(instantiation_id) => {
                for &instance_id in module.get(instantiation_id).instances.iter() {
                    let hir = module.get(instance_id);
                    let src = src_map.get(instance_id);
                    collector.push_symbol(&hir.name, src);
                    collector.pop();
                }
            }
            ModuleItem::ProcId(proc_id) => {
                let proc = module.get(proc_id);
                let stmt_id = proc.stmt;
                build_stmt(db, collector, stmt_id, module, src_map);
            }
            ModuleItem::PortDeclId(port_decl) => {
                let port_decl = module.get(port_decl);
                build_decls(collector, &port_decl.decls, SymbolKind::PortDecl, module, src_map)
            }
            ModuleItem::ContAssignId(_) => {}
            ModuleItem::TypedefId(typedef_id) => build_typedef(collector, typedef_id, module, src_map),
            ModuleItem::StructId(_) | ModuleItem::SubroutineId(_) => {
                // TODO: implement document symbols for these items
            }
        }
    }
    collector.pop();
    regions.finish_all(collector);
}

fn collect_block_items(
    db: &RootDb,
    collector: &mut SymbolCollecter,
    block_id: BlockId,
    block_src: BlockSrc,
) {
    let (block, src_map) = db.block_with_source_map(block_id);
    let (block, src_map) = (block.as_ref(), src_map.as_ref());
    let mut regions = src_map.region_tree.walk().peekable();

    collector.push_symbol_with_children(
        &block.name,
        block_src,
        block.decls.len() + src_map.items.len(),
    );

    for item in src_map.items.iter() {
        regions.add_region_symbol(src_map.item_to_ptr(item).range(), collector);
        match *item {
            BlockItem::DeclarationId(declaration_id) => {
                build_declaration(collector, declaration_id, block, src_map)
            }
            BlockItem::StmtId(stmt_id) => build_stmt(db, collector, stmt_id, block, src_map),
            BlockItem::TypedefId(typedef_id) => build_typedef(collector, typedef_id, block, src_map),
            BlockItem::StructId(_) => {
                // TODO: implement document symbols for these items
            }
        }
    }
    collector.pop();
    regions.finish_all(collector);
}

fn build_stmt<Arn, SrcMap>(
    db: &RootDb,
    collector: &mut SymbolCollecter,
    stmt_id: Idx<Stmt>,
    arena: &Arn,
    src_map: &SrcMap,
) where
    Arn: GetRef<StmtId, Output = Stmt>
        + GetRef<DeclId, Output = Declarator>
        + GetRef<LocalBlockId, Output = BlockInfo>,
    SrcMap: Get<StmtId, Output = StmtSrc>
        + Get<DeclId, Output = DeclaratorSrc>
        + Get<LocalBlockId, Output = BlockSrc>,
{
    let stmt = arena.get(stmt_id);

    if let StmtKind::Block(block_info) = &stmt.kind {
        let block_id = block_info.block_id;
        let block_src = src_map.get(stmt_id).try_into().unwrap();
        collect_block_items(db, collector, block_id, block_src);
        return;
    }

    collector.push_symbol(&stmt.label, src_map.get(stmt_id));
    match &stmt.kind {
        StmtKind::Wait(_, stmt_id)
        | StmtKind::TimingCtrl(_, stmt_id)
        | StmtKind::Forever(stmt_id)
        | StmtKind::DoWhile(stmt_id, _)
        | StmtKind::While(_, stmt_id) => build_stmt(db, collector, *stmt_id, arena, src_map),
        StmtKind::Cond { then_stmt, else_stmt, .. } => {
            build_stmt(db, collector, *then_stmt, arena, src_map);
            if let Some(else_stmt) = else_stmt {
                build_stmt(db, collector, *else_stmt, arena, src_map);
            }
        }
        StmtKind::Case { items, .. } => {
            for item in items {
                let stmt_id = match item {
                    CaseItem::Case { clause, .. } => clause,
                    CaseItem::Default(stmt) => stmt,
                };
                build_stmt(db, collector, *stmt_id, arena, src_map);
            }
        }
        StmtKind::For { inits, stmt, .. } => match inits {
            ForInit::Init(inits) => {
                for (_, decl_id) in inits {
                    build_decl(collector, *decl_id, SymbolKind::DataDecl, arena, src_map);
                }
                build_stmt(db, collector, *stmt, arena, src_map);
            }
            ForInit::Assign(_) => {}
        },

        StmtKind::Empty
        | StmtKind::Expr(_)
        | StmtKind::Jump(_)
        | StmtKind::ProcAssign(_)
        | StmtKind::Disable(_) => {}

        StmtKind::Block(_) => unreachable!(),
    }
    collector.pop();
}

#[inline]
fn build_declaration<Arn, SrcMap>(
    collector: &mut SymbolCollecter,
    declaration_id: DeclarationId,
    arena: &Arn,
    src_map: &SrcMap,
) where
    Arn: GetRef<DeclId, Output = Declarator> + GetRef<DeclarationId, Output = Declaration>,
    SrcMap: Get<DeclId, Output = DeclaratorSrc> + Get<DeclarationId, Output = DeclarationSrc>,
{
    let declaration = arena.get(declaration_id);
    let src = src_map.get(declaration_id);
    build_decls(
        collector,
        &declaration.decls(),
        SymbolKind::from_syntax_kind(src.kind()),
        arena,
        src_map,
    );
}

#[inline]
fn build_decls<Arn, SrcMap>(
    collector: &mut SymbolCollecter,
    decls: &DeclsRange,
    kind: SymbolKind,
    arena: &Arn,
    src_map: &SrcMap,
) where
    Arn: GetRef<DeclId, Output = Declarator>,
    SrcMap: Get<DeclId, Output = DeclaratorSrc>,
{
    for decl in decls.clone() {
        build_decl(collector, decl, kind, arena, src_map);
    }
}

#[inline]
fn build_decl<Arn, SrcMap>(
    collector: &mut SymbolCollecter,
    decl: Idx<Declarator>,
    kind: SymbolKind,
    arena: &Arn,
    src_map: &SrcMap,
) where
    Arn: GetRef<DeclId, Output = Declarator>,
    SrcMap: Get<DeclId, Output = DeclaratorSrc>,
{
    let hir = arena.get(decl);
    let src = src_map.get(decl);
    collector.push_symbol_with_kind(&hir.name, src, kind);
    collector.pop();
}

#[inline]
fn build_typedef<Arn, SrcMap>(
    collector: &mut SymbolCollecter,
    typedef_id: Idx<Typedef>,
    arena: &Arn,
    src_map: &SrcMap,
) where
    Arn: GetRef<TypedefId, Output = Typedef>,
    SrcMap: Get<TypedefId, Output = TypedefSrc>,
{
    let hir = arena.get(typedef_id);
    let src = src_map.get(typedef_id);
    collector.push_symbol_with_kind(&hir.name, src, SymbolKind::Typedef);
    collector.pop();
}
