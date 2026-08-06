use std::iter::Peekable;

use hir_def::{
    DEFAULT_NAME,
    aggregate::{StructDef, StructId, StructKind},
    block::{BlockId, BlockItem},
    checker::{CheckerDef, CheckerId},
    container::InFile,
    covergroup::{CovergroupDef, CovergroupId, CoverpointDef, CoverpointId, CrossDef, CrossId},
    declaration::{Declaration, DeclarationId},
    expr::declarator::{DeclId, Declarator, DeclsRange},
    file::{
        FileItem,
        config::{ConfigDecl, ConfigDeclId},
        library::{LibraryDecl, LibraryDeclId},
        udp::{UdpDecl, UdpDeclId},
    },
    has_source::HasSource,
    module::{
        ModuleId, ModuleItem,
        clocking::{ClockingBlockDef, ClockingBlockId},
        generate::{
            GenerateBlockId, GenerateBlockItem, GenerateItem, GenerateRegion, GenerateRegionId,
        },
        instantiation::{Instance, InstanceId, Instantiation, InstantiationId},
        port::Ports,
        specify::{SpecifyBlock, SpecifyBlockId, SpecifyBlockItem},
    },
    proc::{Proc, ProcId},
    region_tree::{RegionNode, RegionTreeIterator},
    source_map::{HirLookup, NamedSourceLookup, SourceInfo, SourceLookup},
    stmt::{CaseItem, ForInit, Stmt, StmtId, StmtKind},
    subroutine::{LocalSubroutineId, Subroutine},
    typedef::{Typedef, TypedefId},
};
use hir_ty::db::TyDb;
use preproc_expand::file::HirFileId;
use smol_str::SmolStr;
use syntax::WalkEvent;
use utils::line_index::TextRange;
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
struct SymbolCollector {
    res: Vec<DocumentSymbol>,
    stack: Vec<DocumentSymbol>,
}

impl SymbolCollector {
    pub fn new(len: usize) -> Self {
        Self { res: Vec::with_capacity(len), stack: Vec::with_capacity(len) }
    }

    /// Dotted path of the current enclosing symbols (`self.stack`, which holds
    /// every open ancestor but not the symbol being pushed). This is what
    /// qualified workspace-symbol queries (e.g. `mod.block.sig`) filter on,
    /// and is reported as the LSP `containerName`. The path is dotted to match
    /// `workspace_symbols::Query::matches`, which splits on `.`.
    fn container_path(&self) -> Option<String> {
        if self.stack.is_empty() {
            None
        } else {
            Some(self.stack.iter().map(|sym| sym.name.as_str()).collect::<Vec<_>>().join("."))
        }
    }

    pub fn push_symbol(&mut self, name: &Option<SmolStr>, src: SourceInfo) {
        let container_name = self.container_path();
        let sym = DocumentSymbol {
            name: name.as_ref().unwrap_or(&DEFAULT_NAME).to_string(),
            focus_range: src.focus_or_full_range(),
            full_range: src.full_range(),
            kind: SymbolKind::from_syntax_kind(
                src.kind().expect("mapped source should retain its syntax kind"),
            ),
            detail: None,
            container_name,
            children: Vec::new(),
        };
        self.stack.push(sym);
    }

    pub fn push_symbol_with_kind(
        &mut self,
        name: &Option<SmolStr>,
        src: SourceInfo,
        kind: SymbolKind,
    ) {
        self.push_symbol(name, src);
        if let Some(symbol) = self.stack.last_mut() {
            symbol.kind = kind;
        }
    }

    pub fn push_symbol_with_children(
        &mut self,
        name: &Option<SmolStr>,
        src: SourceInfo,
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
        let container_name = self.container_path();
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
        let Some(mut sym) = self.stack.pop() else {
            return;
        };

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

    pub fn finish(mut self) -> Vec<DocumentSymbol> {
        while !self.stack.is_empty() {
            self.pop();
        }
        self.res
    }
}

trait AddRegionSymbol {
    fn add_region_symbol(&mut self, node_range: TextRange, collector: &mut SymbolCollector);
    fn finish_all(&mut self, collector: &mut SymbolCollector);
}

impl AddRegionSymbol for Peekable<RegionTreeIterator<'_>> {
    #[inline]
    fn add_region_symbol<'a>(&mut self, node_range: TextRange, collector: &mut SymbolCollector) {
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
            let _ = self.next();
        }
    }

    #[inline]
    fn finish_all(&mut self, collector: &mut SymbolCollector) {
        for event in self {
            match event {
                WalkEvent::Enter(region) => collector.push_region(region),
                WalkEvent::Leave(_) => collector.pop(),
            }
        }
    }
}

// TODO: add ty info in detail
pub(crate) fn document_symbols(db: &dyn TyDb, file_id: FileId) -> Vec<DocumentSymbol> {
    let _span = tracing::debug_span!("ide.document_symbols", ?file_id).entered();
    let file_id = HirFileId::File(file_id);
    let lowered = db.hir_file_with_source_map(file_id);
    let file = lowered.data_ref();
    let src_map = lowered.source_map();
    let mut regions = src_map.region_tree.walk().peekable();

    let mut collector = SymbolCollector::new(
        src_map.items.len() + src_map.region_tree.root_count() + file.decls.len(),
    );

    for &item in src_map.items.iter() {
        if let Some(ptr) = src_map.item_to_ptr(&item) {
            regions.add_region_symbol(ptr.range(), &mut collector);
        }

        match item {
            FileItem::LocalModuleId(idx) => {
                collect_module_items(db, ModuleId::new(file_id, idx), &mut collector);
            }
            FileItem::ProcId(proc_id) => {
                let stmt_id = lowered.get(proc_id).stmt;
                build_stmt(db, &mut collector, stmt_id, lowered.as_ref());
            }
            FileItem::DeclarationId(declaration_id) => {
                build_declaration(&mut collector, declaration_id, lowered.as_ref());
            }
            FileItem::TypedefId(typedef_id) => {
                build_typedef(&mut collector, typedef_id, lowered.as_ref())
            }
            FileItem::SubroutineId(subroutine_id) => {
                build_subroutine(&mut collector, subroutine_id, lowered.as_ref())
            }
            FileItem::StructId(struct_id) => {
                build_struct(&mut collector, struct_id, lowered.as_ref())
            }
            FileItem::ConfigDeclId(config_id) => {
                build_config_decl(&mut collector, config_id, lowered.as_ref())
            }
            FileItem::LibraryDeclId(library_id) => {
                build_library_decl(&mut collector, library_id, lowered.as_ref())
            }
            FileItem::LibraryIncludeId(_) => {}
            FileItem::CheckerId(checker_id) => {
                build_checker(&mut collector, checker_id, lowered.as_ref())
            }
            FileItem::CovergroupId(covergroup_id) => {
                build_covergroup(&mut collector, covergroup_id, lowered.as_ref())
            }
            FileItem::UdpDeclId(udp_id) => build_udp_decl(&mut collector, udp_id, lowered.as_ref()),
        }
    }

    regions.finish_all(&mut collector);
    collector.finish()
}

fn collect_module_items(db: &dyn TyDb, module_id: ModuleId, collector: &mut SymbolCollector) {
    let lowered = db.module_with_source_map(module_id);
    let module = lowered.data_ref();
    let src_map = lowered.source_map();
    let mut regions = src_map.region_tree.walk().peekable();

    let Some(InFile { value: module_src, .. }) = module_id.source(db) else {
        return;
    };
    collector.push_symbol_with_children(
        &module.name,
        module_src,
        src_map.items.len() + module.decls.len() + module.stmts.len(),
    );

    if let Some(params) = &module.param_ports {
        for decl_id in params.clone() {
            if let Some(src) = lowered.source_info(decl_id) {
                regions.add_region_symbol(src.full_range(), collector);
            }
            build_decl(collector, decl_id, SymbolKind::ParamDecl, lowered.as_ref());
        }
    }

    match &module.ports {
        Ports::NonAnsi { ports, .. } => {
            for (port_id, port) in ports.iter() {
                if let Some(src) = lowered.named_source_info(port_id) {
                    regions.add_region_symbol(src.full_range(), collector);
                    collector.push_symbol(&port.label, src);
                    collector.pop();
                }
            }
        }
        Ports::Ansi(port_decls) => {
            for (port_id, port_decl) in port_decls.iter() {
                if let Some(src) = lowered.source_info(port_id) {
                    regions.add_region_symbol(src.full_range(), collector);
                }
                build_decls(collector, &port_decl.decls, SymbolKind::PortDecl, lowered.as_ref());
            }
        }
    }

    for item in src_map.items.iter() {
        if let Some(ptr) = src_map.item_to_ptr(item) {
            regions.add_region_symbol(ptr.range(), collector);
        }
        match *item {
            ModuleItem::DeclarationId(declaration_id) => {
                build_declaration(collector, declaration_id, lowered.as_ref())
            }
            ModuleItem::InstantiationId(instantiation_id) => {
                for &instance_id in lowered.get(instantiation_id).instances.iter() {
                    let hir = lowered.get(instance_id);
                    if let Some(src) = lowered.named_source_info(instance_id) {
                        collector.push_symbol(&hir.name, src);
                        collector.pop();
                    }
                }
            }
            ModuleItem::ProcId(proc_id) => {
                let stmt_id = lowered.get(proc_id).stmt;
                build_stmt(db, collector, stmt_id, lowered.as_ref());
            }
            ModuleItem::PortDeclId(port_decl) => {
                let port_decl = lowered.get(port_decl);
                build_decls(collector, &port_decl.decls, SymbolKind::PortDecl, lowered.as_ref())
            }
            ModuleItem::ContAssignId(_) => {}
            ModuleItem::DefParamId(_) => {}
            ModuleItem::GenerateRegionId(generate_region_id) => {
                build_generate_region(db, collector, generate_region_id, lowered.as_ref())
            }
            ModuleItem::SpecifyBlockId(specify_block_id) => {
                build_specify_block(collector, specify_block_id, lowered.as_ref())
            }
            ModuleItem::SpecifyItemId(_) => {}
            ModuleItem::TypedefId(typedef_id) => {
                build_typedef(collector, typedef_id, lowered.as_ref())
            }
            ModuleItem::SubroutineId(subroutine_id) => {
                build_subroutine(collector, subroutine_id, lowered.as_ref())
            }
            ModuleItem::ModportId(modport_id) => {
                let modport = lowered.get(modport_id);
                if let Some(src) = lowered.named_source_info(modport_id) {
                    collector.push_symbol(&modport.name, src);
                    collector.pop();
                }
            }
            ModuleItem::ClockingBlockId(clocking_block_id) => {
                build_clocking_block(collector, clocking_block_id, lowered.as_ref());
            }
            ModuleItem::CheckerId(checker_id) => {
                build_checker(collector, checker_id, lowered.as_ref());
            }
            ModuleItem::CovergroupId(covergroup_id) => {
                build_covergroup(collector, covergroup_id, lowered.as_ref());
            }
            ModuleItem::StructId(struct_id) => build_struct(collector, struct_id, lowered.as_ref()),
        }
    }
    collector.pop();
    regions.finish_all(collector);
}

fn collect_block_items(db: &dyn TyDb, collector: &mut SymbolCollector, block_id: BlockId) {
    let lowered = db.block_with_source_map(block_id.clone());
    let block = lowered.data_ref();
    let src_map = lowered.source_map();
    let mut regions = src_map.region_tree.walk().peekable();

    let Some(InFile { value: block_src, .. }) = block_id.source(db) else {
        return;
    };
    collector.push_symbol_with_children(
        &block.name,
        block_src,
        block.decls.len() + src_map.items.len(),
    );

    for item in src_map.items.iter() {
        if let Some(ptr) = src_map.item_to_ptr(item) {
            regions.add_region_symbol(ptr.range(), collector);
        }
        match *item {
            BlockItem::DeclarationId(declaration_id) => {
                build_declaration(collector, declaration_id, lowered.as_ref())
            }
            BlockItem::StmtId(stmt_id) => build_stmt(db, collector, stmt_id, lowered.as_ref()),
            BlockItem::TypedefId(typedef_id) => {
                build_typedef(collector, typedef_id, lowered.as_ref())
            }
            BlockItem::StructId(struct_id) => build_struct(collector, struct_id, lowered.as_ref()),
        }
    }
    collector.pop();
    regions.finish_all(collector);
}
fn build_stmt<L>(db: &dyn TyDb, collector: &mut SymbolCollector, stmt_id: StmtId, lowered: &L)
where
    L: HirLookup<StmtId, Hir = Stmt>
        + HirLookup<DeclId, Hir = Declarator>
        + NamedSourceLookup<StmtId>
        + NamedSourceLookup<DeclId>,
{
    let stmt = lowered.hir(stmt_id);

    if let StmtKind::Block(block_info) = &stmt.kind {
        collect_block_items(db, collector, block_info.block_id.clone());
        return;
    }

    let Some(stmt_src) = lowered.named_source_info(stmt_id) else {
        return;
    };
    collector.push_symbol(&stmt.label, stmt_src);
    match &stmt.kind {
        StmtKind::Wait(_, stmt_id)
        | StmtKind::TimingCtrl(_, stmt_id)
        | StmtKind::Forever(stmt_id)
        | StmtKind::DoWhile(stmt_id, _)
        | StmtKind::Repeat(_, stmt_id)
        | StmtKind::While(_, stmt_id) => build_stmt(db, collector, *stmt_id, lowered),
        StmtKind::Cond { then_stmt, else_stmt, .. } => {
            build_stmt(db, collector, *then_stmt, lowered);
            if let Some(else_stmt) = else_stmt {
                build_stmt(db, collector, *else_stmt, lowered);
            }
        }
        StmtKind::Case { items, .. } => {
            for item in items {
                let stmt_id = match item {
                    CaseItem::Case { clause, .. } => clause,
                    CaseItem::Default(stmt) => stmt,
                };
                build_stmt(db, collector, *stmt_id, lowered);
            }
        }
        StmtKind::For { inits, stmt, .. } => {
            if let ForInit::Init(inits) = inits {
                for (_, decl_id) in inits {
                    build_decl(collector, *decl_id, SymbolKind::DataDecl, lowered);
                }
            }
            build_stmt(db, collector, *stmt, lowered);
        }

        StmtKind::Missing
        | StmtKind::Invalid
        | StmtKind::Unsupported(_)
        | StmtKind::Empty
        | StmtKind::Expr(_)
        | StmtKind::Jump(_)
        | StmtKind::EventTrigger(_)
        | StmtKind::ProcAssign(_)
        | StmtKind::Disable(_) => {}

        StmtKind::Block(_) => {}
    }
    collector.pop();
}

fn build_declaration<L>(collector: &mut SymbolCollector, declaration_id: DeclarationId, lowered: &L)
where
    L: HirLookup<DeclId, Hir = Declarator>
        + HirLookup<DeclarationId, Hir = Declaration>
        + NamedSourceLookup<DeclId>
        + SourceLookup<DeclarationId>,
{
    let declaration = lowered.hir(declaration_id);
    let Some(src) = lowered.source_info(declaration_id) else {
        return;
    };
    build_decls(
        collector,
        &declaration.decls(),
        SymbolKind::from_syntax_kind(
            src.kind().expect("mapped source should retain its syntax kind"),
        ),
        lowered,
    );
}

#[inline]
fn build_generate_region<L>(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    generate_region_id: GenerateRegionId,
    lowered: &L,
) where
    L: HirLookup<GenerateRegionId, Hir = GenerateRegion>
        + HirLookup<DeclarationId, Hir = Declaration>
        + HirLookup<DeclId, Hir = Declarator>
        + HirLookup<InstanceId, Hir = Instance>
        + HirLookup<InstantiationId, Hir = Instantiation>
        + HirLookup<LocalSubroutineId, Hir = Subroutine>
        + HirLookup<ProcId, Hir = Proc>
        + HirLookup<StmtId, Hir = Stmt>
        + HirLookup<StructId, Hir = StructDef>
        + HirLookup<TypedefId, Hir = Typedef>
        + NamedSourceLookup<GenerateRegionId>
        + SourceLookup<DeclarationId>
        + NamedSourceLookup<DeclId>
        + NamedSourceLookup<InstanceId>
        + NamedSourceLookup<LocalSubroutineId>
        + NamedSourceLookup<StmtId>
        + NamedSourceLookup<StructId>
        + NamedSourceLookup<TypedefId>,
{
    let hir = lowered.hir(generate_region_id);
    let Some(src) = lowered.named_source_info(generate_region_id) else {
        return;
    };
    let name = Some(SmolStr::new_static("generate"));
    collector.push_symbol_with_kind(&name, src, SymbolKind::Generate);
    for item in hir.items.iter() {
        build_generate_block_item(
            db,
            collector,
            generate_item_to_block_item(item.clone()),
            lowered,
        );
    }
    collector.pop();
}

fn build_generate_block(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    generate_block_id: GenerateBlockId,
) {
    let Some(InFile { value: generate_block_src, .. }) = generate_block_id.source(db) else {
        return;
    };
    let lowered = db.generate_block_with_source_map(generate_block_id);
    let generate_block = lowered.data_ref();
    let name = generate_block.name.clone();

    collector.push_symbol_with_kind(&name, generate_block_src, SymbolKind::Generate);
    for item in &generate_block.items {
        build_generate_block_item(db, collector, item.clone(), lowered.as_ref());
    }
    collector.pop();
}

/// Builds a document symbol for one generate item, shared by generate regions
/// (whose items live in the enclosing module's arenas) and generate blocks
/// (whose items live in their own container).
#[inline]
fn build_generate_block_item<L>(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    item: GenerateBlockItem,
    lowered: &L,
) where
    L: HirLookup<DeclarationId, Hir = Declaration>
        + HirLookup<DeclId, Hir = Declarator>
        + HirLookup<InstanceId, Hir = Instance>
        + HirLookup<InstantiationId, Hir = Instantiation>
        + HirLookup<LocalSubroutineId, Hir = Subroutine>
        + HirLookup<ProcId, Hir = Proc>
        + HirLookup<StmtId, Hir = Stmt>
        + HirLookup<StructId, Hir = StructDef>
        + HirLookup<TypedefId, Hir = Typedef>
        + SourceLookup<DeclarationId>
        + NamedSourceLookup<DeclId>
        + NamedSourceLookup<InstanceId>
        + NamedSourceLookup<LocalSubroutineId>
        + NamedSourceLookup<StmtId>
        + NamedSourceLookup<StructId>
        + NamedSourceLookup<TypedefId>,
{
    match item {
        GenerateBlockItem::ContAssignId(_) | GenerateBlockItem::DefParamId(_) => {}
        GenerateBlockItem::DeclarationId(declaration_id) => {
            build_declaration(collector, declaration_id, lowered);
        }
        GenerateBlockItem::GenerateBlockId(child_id) => {
            build_generate_block(db, collector, child_id);
        }
        GenerateBlockItem::TypedefId(typedef_id) => {
            build_typedef(collector, typedef_id, lowered);
        }
        GenerateBlockItem::SubroutineId(subroutine_id) => {
            build_subroutine(collector, subroutine_id, lowered);
        }
        GenerateBlockItem::ProcId(proc_id) => {
            let proc = lowered.hir(proc_id);
            build_stmt(db, collector, proc.stmt, lowered);
        }
        GenerateBlockItem::InstantiationId(instantiation_id) => {
            for &instance_id in lowered.hir(instantiation_id).instances.iter() {
                let hir = lowered.hir(instance_id);
                if let Some(src) = lowered.named_source_info(instance_id) {
                    collector.push_symbol(&hir.name, src);
                    collector.pop();
                }
            }
        }
        GenerateBlockItem::StructId(struct_id) => {
            build_struct(collector, struct_id, lowered);
        }
    }
}

fn generate_item_to_block_item(item: GenerateItem) -> GenerateBlockItem {
    match item {
        GenerateItem::ContAssignId(id) => GenerateBlockItem::ContAssignId(id),
        GenerateItem::DefParamId(id) => GenerateBlockItem::DefParamId(id),
        GenerateItem::GenerateBlockId(id) => GenerateBlockItem::GenerateBlockId(id),
        GenerateItem::DeclarationId(id) => GenerateBlockItem::DeclarationId(id),
        GenerateItem::StructId(id) => GenerateBlockItem::StructId(id),
        GenerateItem::InstantiationId(id) => GenerateBlockItem::InstantiationId(id),
        GenerateItem::ProcId(id) => GenerateBlockItem::ProcId(id),
        GenerateItem::TypedefId(id) => GenerateBlockItem::TypedefId(id),
        GenerateItem::SubroutineId(id) => GenerateBlockItem::SubroutineId(id),
    }
}

#[inline]
fn build_checker<L>(collector: &mut SymbolCollector, checker_id: CheckerId, lowered: &L)
where
    L: HirLookup<CheckerId, Hir = CheckerDef> + NamedSourceLookup<CheckerId>,
{
    let checker = lowered.hir(checker_id);
    let Some(src) = lowered.named_source_info(checker_id) else {
        return;
    };
    collector.push_symbol(&checker.name, src);
    collector.pop();
}

#[inline]
fn build_clocking_block<L>(
    collector: &mut SymbolCollector,
    clocking_block_id: ClockingBlockId,
    lowered: &L,
) where
    L: HirLookup<ClockingBlockId, Hir = ClockingBlockDef> + NamedSourceLookup<ClockingBlockId>,
{
    let clocking_block = lowered.hir(clocking_block_id);
    let Some(src) = lowered.named_source_info(clocking_block_id) else {
        return;
    };
    collector.push_symbol(&clocking_block.name, src);
    collector.pop();
}

#[inline]
fn build_covergroup<L>(collector: &mut SymbolCollector, covergroup_id: CovergroupId, lowered: &L)
where
    L: HirLookup<CovergroupId, Hir = CovergroupDef>
        + HirLookup<CoverpointId, Hir = CoverpointDef>
        + HirLookup<CrossId, Hir = CrossDef>
        + NamedSourceLookup<CovergroupId>
        + NamedSourceLookup<CoverpointId>
        + NamedSourceLookup<CrossId>,
{
    let covergroup = lowered.hir(covergroup_id);
    let Some(src) = lowered.named_source_info(covergroup_id) else {
        return;
    };
    collector.push_symbol_with_children(
        &covergroup.name,
        src,
        covergroup.coverpoints.len() + covergroup.crosses.len(),
    );
    for &coverpoint_id in &covergroup.coverpoints {
        build_coverpoint(collector, coverpoint_id, lowered);
    }
    for &cross_id in &covergroup.crosses {
        build_cross(collector, cross_id, lowered);
    }
    collector.pop();
}

#[inline]
fn build_coverpoint<L>(collector: &mut SymbolCollector, coverpoint_id: CoverpointId, lowered: &L)
where
    L: HirLookup<CoverpointId, Hir = CoverpointDef> + NamedSourceLookup<CoverpointId>,
{
    let coverpoint = lowered.hir(coverpoint_id);
    let Some(src) = lowered.named_source_info(coverpoint_id) else {
        return;
    };
    collector.push_symbol(&coverpoint.name, src);
    collector.pop();
}

#[inline]
fn build_cross<L>(collector: &mut SymbolCollector, cross_id: CrossId, lowered: &L)
where
    L: HirLookup<CrossId, Hir = CrossDef> + NamedSourceLookup<CrossId>,
{
    let cross = lowered.hir(cross_id);
    let Some(src) = lowered.named_source_info(cross_id) else {
        return;
    };
    collector.push_symbol(&cross.name, src);
    collector.pop();
}

#[inline]
fn build_struct<L>(collector: &mut SymbolCollector, struct_id: StructId, lowered: &L)
where
    L: HirLookup<StructId, Hir = StructDef> + NamedSourceLookup<StructId>,
{
    let hir = lowered.hir(struct_id);
    let Some(src) = lowered.named_source_info(struct_id) else {
        return;
    };

    let name = hir.name.clone().or_else(|| Some(struct_kind_name(hir.kind)));
    collector.push_symbol_with_kind(&name, src, SymbolKind::Struct);
    collector.pop();
}

#[inline]
fn struct_kind_name(kind: StructKind) -> SmolStr {
    match kind {
        StructKind::Struct => SmolStr::new_static("struct"),
        StructKind::Union => SmolStr::new_static("union"),
    }
}

#[inline]
fn build_specify_block<L>(
    collector: &mut SymbolCollector,
    specify_block_id: SpecifyBlockId,
    lowered: &L,
) where
    L: HirLookup<SpecifyBlockId, Hir = SpecifyBlock>
        + HirLookup<DeclarationId, Hir = Declaration>
        + HirLookup<DeclId, Hir = Declarator>
        + NamedSourceLookup<SpecifyBlockId>
        + SourceLookup<DeclarationId>
        + NamedSourceLookup<DeclId>,
{
    let hir = lowered.hir(specify_block_id);
    let Some(src) = lowered.named_source_info(specify_block_id) else {
        return;
    };
    let name = Some(SmolStr::new_static("specify"));
    collector.push_symbol_with_kind(&name, src, SymbolKind::Specify);
    for item in hir.items.iter() {
        match *item {
            SpecifyBlockItem::DeclarationId(declaration_id) => {
                build_declaration(collector, declaration_id, lowered);
            }
            SpecifyBlockItem::SpecifyItemId(_) => {}
        }
    }
    collector.pop();
}

#[inline]
fn build_decls<L>(
    collector: &mut SymbolCollector,
    decls: &DeclsRange,
    kind: SymbolKind,
    lowered: &L,
) where
    L: HirLookup<DeclId, Hir = Declarator> + NamedSourceLookup<DeclId>,
{
    for decl in decls.clone() {
        build_decl(collector, decl, kind, lowered);
    }
}

#[inline]
fn build_decl<L>(collector: &mut SymbolCollector, decl: DeclId, kind: SymbolKind, lowered: &L)
where
    L: HirLookup<DeclId, Hir = Declarator> + NamedSourceLookup<DeclId>,
{
    let hir = lowered.hir(decl);
    let Some(src) = lowered.named_source_info(decl) else {
        return;
    };
    collector.push_symbol_with_kind(&hir.name, src, kind);
    collector.pop();
}

#[inline]
fn build_typedef<L>(collector: &mut SymbolCollector, typedef_id: TypedefId, lowered: &L)
where
    L: HirLookup<TypedefId, Hir = Typedef> + NamedSourceLookup<TypedefId>,
{
    let hir = lowered.hir(typedef_id);
    let Some(src) = lowered.named_source_info(typedef_id) else {
        return;
    };
    let kind = match hir.ty {
        Some(hir_def::expr::data_ty::DataTy::Struct(_)) => SymbolKind::Struct,
        _ => SymbolKind::Typedef,
    };
    collector.push_symbol_with_kind(&hir.name, src, kind);
    collector.pop();
}

#[inline]
fn build_subroutine<L>(
    collector: &mut SymbolCollector,
    subroutine_id: LocalSubroutineId,
    lowered: &L,
) where
    L: HirLookup<LocalSubroutineId, Hir = Subroutine> + NamedSourceLookup<LocalSubroutineId>,
{
    let hir = lowered.hir(subroutine_id);
    let Some(src) = lowered.named_source_info(subroutine_id) else {
        return;
    };
    collector.push_symbol_with_kind(&hir.name, src, SymbolKind::Fn);
    collector.pop();
}

#[inline]
fn build_config_decl<L>(collector: &mut SymbolCollector, config_id: ConfigDeclId, lowered: &L)
where
    L: HirLookup<ConfigDeclId, Hir = ConfigDecl> + NamedSourceLookup<ConfigDeclId>,
{
    let hir = lowered.hir(config_id);
    let Some(src) = lowered.named_source_info(config_id) else {
        return;
    };
    collector.push_symbol_with_kind(&hir.name, src, SymbolKind::Config);
    collector.pop();
}

#[inline]
fn build_udp_decl<L>(collector: &mut SymbolCollector, udp_id: UdpDeclId, lowered: &L)
where
    L: HirLookup<UdpDeclId, Hir = UdpDecl> + NamedSourceLookup<UdpDeclId>,
{
    let hir = lowered.hir(udp_id);
    let Some(src) = lowered.named_source_info(udp_id) else {
        return;
    };
    collector.push_symbol_with_kind(&hir.name, src, SymbolKind::Primitive);
    collector.pop();
}

#[inline]
fn build_library_decl<L>(collector: &mut SymbolCollector, library_id: LibraryDeclId, lowered: &L)
where
    L: HirLookup<LibraryDeclId, Hir = LibraryDecl> + NamedSourceLookup<LibraryDeclId>,
{
    let hir = lowered.hir(library_id);
    let Some(src) = lowered.named_source_info(library_id) else {
        return;
    };
    collector.push_symbol_with_kind(&hir.name, src, SymbolKind::Library);
    collector.pop();
}
