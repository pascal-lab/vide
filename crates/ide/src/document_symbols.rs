use std::iter::Peekable;

use base_db::intern::Lookup;
use hir::{
    container::InFile,
    db::HirDb,
    file::HirFileId,
    hir_def::{
        DEFAULT_NAME,
        block::{BlockId, BlockInfo, BlockItem, BlockSrc, LocalBlockId},
        declaration::{Declaration, DeclarationId, DeclarationSrc},
        expr::declarator::{DeclId, Declarator, DeclaratorSrc, DeclsRange},
        file::{
            FileItem,
            config::{ConfigDecl, ConfigDeclId, ConfigDeclSrc},
            udp::{UdpDecl, UdpDeclId, UdpDeclSrc},
        },
        module::{
            ModuleId, ModuleItem, ModuleSrc,
            generate::{
                GenerateBlockId, GenerateBlockItem, GenerateBlockLoc, GenerateItem, GenerateRegion,
                GenerateRegionId, GenerateRegionSrc,
            },
            port::Ports,
            specify::{SpecifyBlock, SpecifyBlockId, SpecifyBlockItem, SpecifyBlockSrc},
        },
        opaque::{OpaqueItem, OpaqueItemId, OpaqueItemSrc},
        stmt::{CaseItem, ForInit, Stmt, StmtId, StmtKind, StmtSrc},
        subroutine::{LocalSubroutineId, Subroutine, SubroutineSrc},
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
        assert!(self.stack.is_empty(), "{:?}", self.stack);
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
            FileItem::TypedefId(typedef_id) => {
                build_typedef(&mut collector, typedef_id, file, src_map)
            }
            FileItem::OpaqueItemId(opaque_id) => {
                build_opaque(&mut collector, opaque_id, file, src_map)
            }
            FileItem::SubroutineId(subroutine_id) => {
                build_subroutine(&mut collector, subroutine_id, file, src_map)
            }
            FileItem::StructId(_) => {
                // TODO: implement document symbols for these items
            }
            FileItem::ConfigDeclId(config_id) => {
                build_config_decl(&mut collector, config_id, file, src_map)
            }
            FileItem::UdpDeclId(udp_id) => build_udp_decl(&mut collector, udp_id, file, src_map),
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
            ModuleItem::DefParamId(_) => {}
            ModuleItem::GenerateRegionId(generate_region_id) => {
                build_generate_region(db, collector, generate_region_id, module, src_map)
            }
            ModuleItem::SpecifyBlockId(specify_block_id) => {
                build_specify_block(collector, specify_block_id, module, src_map)
            }
            ModuleItem::SpecifyItemId(_) => {}
            ModuleItem::TypedefId(typedef_id) => {
                build_typedef(collector, typedef_id, module, src_map)
            }
            ModuleItem::OpaqueItemId(opaque_id) => {
                build_opaque(collector, opaque_id, module, src_map)
            }
            ModuleItem::SubroutineId(subroutine_id) => {
                build_subroutine(collector, subroutine_id, module, src_map)
            }
            ModuleItem::StructId(_) => {
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
            BlockItem::TypedefId(typedef_id) => {
                build_typedef(collector, typedef_id, block, src_map)
            }
            BlockItem::OpaqueItemId(opaque_id) => {
                build_opaque(collector, opaque_id, block, src_map)
            }
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
        | StmtKind::EventTrigger(_)
        | StmtKind::ProcAssign(_)
        | StmtKind::Disable(_)
        | StmtKind::Opaque => {}

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
fn build_generate_region<Arn, SrcMap>(
    db: &RootDb,
    collector: &mut SymbolCollecter,
    generate_region_id: GenerateRegionId,
    arena: &Arn,
    src_map: &SrcMap,
) where
    Arn: GetRef<GenerateRegionId, Output = GenerateRegion>
        + GetRef<DeclarationId, Output = Declaration>
        + GetRef<DeclId, Output = Declarator>
        + GetRef<OpaqueItemId, Output = OpaqueItem>,
    SrcMap: Get<GenerateRegionId, Output = GenerateRegionSrc>
        + Get<DeclarationId, Output = DeclarationSrc>
        + Get<DeclId, Output = DeclaratorSrc>
        + Get<OpaqueItemId, Output = OpaqueItemSrc>,
{
    let hir = arena.get(generate_region_id);
    let src = src_map.get(generate_region_id);
    let name = Some(SmolStr::new_static("generate"));
    collector.push_symbol_with_kind(&name, src, SymbolKind::Generate);
    for item in hir.items.iter() {
        match *item {
            GenerateItem::DeclarationId(declaration_id) => {
                build_declaration(collector, declaration_id, arena, src_map);
            }
            GenerateItem::GenerateBlockId(generate_block_id) => {
                build_generate_block(db, collector, generate_block_id);
            }
            GenerateItem::OpaqueItemId(opaque_id) => {
                build_opaque(collector, opaque_id, arena, src_map);
            }
        }
    }
    collector.pop();
}

fn build_generate_block(
    db: &RootDb,
    collector: &mut SymbolCollecter,
    generate_block_id: GenerateBlockId,
) {
    let GenerateBlockLoc { src: InFile { value: generate_block_src, .. }, .. } =
        generate_block_id.lookup(db);
    let (generate_block, src_map) = db.generate_block_with_source_map(generate_block_id);
    let (generate_block, src_map) = (generate_block.as_ref(), src_map.as_ref());
    let name = generate_block.name.clone();

    collector.push_symbol_with_kind(&name, generate_block_src, SymbolKind::Generate);
    for item in &generate_block.items {
        match *item {
            GenerateBlockItem::DeclarationId(declaration_id) => {
                build_declaration(collector, declaration_id, generate_block, src_map);
            }
            GenerateBlockItem::GenerateBlockId(child_id) => {
                build_generate_block(db, collector, child_id);
            }
            GenerateBlockItem::TypedefId(typedef_id) => {
                build_typedef(collector, typedef_id, generate_block, src_map);
            }
            GenerateBlockItem::OpaqueItemId(opaque_id) => {
                build_opaque(collector, opaque_id, generate_block, src_map);
            }
            GenerateBlockItem::SubroutineId(subroutine_id) => {
                build_subroutine(collector, subroutine_id, generate_block, src_map);
            }
            GenerateBlockItem::ProcId(proc_id) => {
                let proc = generate_block.get(proc_id);
                build_stmt(db, collector, proc.stmt, generate_block, src_map);
            }
            GenerateBlockItem::InstantiationId(instantiation_id) => {
                for &instance_id in generate_block.get(instantiation_id).instances.iter() {
                    let hir = generate_block.get(instance_id);
                    let src = src_map.get(instance_id);
                    collector.push_symbol(&hir.name, src);
                    collector.pop();
                }
            }
            GenerateBlockItem::ContAssignId(_)
            | GenerateBlockItem::DefParamId(_)
            | GenerateBlockItem::StructId(_) => {}
        }
    }
    collector.pop();
}

#[inline]
fn build_specify_block<Arn, SrcMap>(
    collector: &mut SymbolCollecter,
    specify_block_id: SpecifyBlockId,
    arena: &Arn,
    src_map: &SrcMap,
) where
    Arn: GetRef<SpecifyBlockId, Output = SpecifyBlock>
        + GetRef<DeclarationId, Output = Declaration>
        + GetRef<DeclId, Output = Declarator>
        + GetRef<OpaqueItemId, Output = OpaqueItem>,
    SrcMap: Get<SpecifyBlockId, Output = SpecifyBlockSrc>
        + Get<DeclarationId, Output = DeclarationSrc>
        + Get<DeclId, Output = DeclaratorSrc>
        + Get<OpaqueItemId, Output = OpaqueItemSrc>,
{
    let hir = arena.get(specify_block_id);
    let src = src_map.get(specify_block_id);
    let name = Some(SmolStr::new_static("specify"));
    collector.push_symbol_with_kind(&name, src, SymbolKind::Specify);
    for item in hir.items.iter() {
        match *item {
            SpecifyBlockItem::DeclarationId(declaration_id) => {
                build_declaration(collector, declaration_id, arena, src_map);
            }
            SpecifyBlockItem::SpecifyItemId(_) => {}
            SpecifyBlockItem::OpaqueItemId(opaque_id) => {
                build_opaque(collector, opaque_id, arena, src_map);
            }
        }
    }
    collector.pop();
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

#[inline]
fn build_subroutine<Arn, SrcMap>(
    collector: &mut SymbolCollecter,
    subroutine_id: LocalSubroutineId,
    arena: &Arn,
    src_map: &SrcMap,
) where
    Arn: GetRef<LocalSubroutineId, Output = Subroutine>,
    SrcMap: Get<LocalSubroutineId, Output = SubroutineSrc>,
{
    let hir = arena.get(subroutine_id);
    let src = src_map.get(subroutine_id);
    collector.push_symbol_with_kind(&hir.name, src, SymbolKind::Fn);
    collector.pop();
}

#[inline]
fn build_config_decl<Arn, SrcMap>(
    collector: &mut SymbolCollecter,
    config_id: ConfigDeclId,
    arena: &Arn,
    src_map: &SrcMap,
) where
    Arn: GetRef<ConfigDeclId, Output = ConfigDecl>,
    SrcMap: Get<ConfigDeclId, Output = ConfigDeclSrc>,
{
    let hir = arena.get(config_id);
    let src = src_map.get(config_id);
    collector.push_symbol_with_kind(&hir.name, src, SymbolKind::Config);
    collector.pop();
}

#[inline]
fn build_udp_decl<Arn, SrcMap>(
    collector: &mut SymbolCollecter,
    udp_id: UdpDeclId,
    arena: &Arn,
    src_map: &SrcMap,
) where
    Arn: GetRef<UdpDeclId, Output = UdpDecl>,
    SrcMap: Get<UdpDeclId, Output = UdpDeclSrc>,
{
    let hir = arena.get(udp_id);
    let src = src_map.get(udp_id);
    collector.push_symbol_with_kind(&hir.name, src, SymbolKind::Primitive);
    collector.pop();
}

#[inline]
fn build_opaque<Arn, SrcMap>(
    collector: &mut SymbolCollecter,
    opaque_id: OpaqueItemId,
    arena: &Arn,
    src_map: &SrcMap,
) where
    Arn: GetRef<OpaqueItemId, Output = OpaqueItem>,
    SrcMap: Get<OpaqueItemId, Output = OpaqueItemSrc>,
{
    let hir = arena.get(opaque_id);
    let src = src_map.get(opaque_id);
    let kind = SymbolKind::from_opaque_kind(hir.kind, src.kind());
    collector.push_symbol_with_kind(&hir.name, src, kind);
    collector.pop();
}
