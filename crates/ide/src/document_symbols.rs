use std::iter::Peekable;

use hir_def::{
    DEFAULT_NAME,
    aggregate::{StructDef, StructId, StructKind},
    assertion::{PropertyDef, PropertyId, SequenceDef, SequenceId},
    block::BlockItem,
    body::{Body, BodyItem},
    checker::{CheckerDef, CheckerId},
    container::InFile,
    covergroup::{CovergroupDef, CovergroupId, CoverpointDef, CoverpointId, CrossDef, CrossId},
    declaration::{Declaration, DeclarationId},
    expr::declarator::{DeclId, Declarator, DeclsRange},
    file::{
        config::{ConfigDecl, ConfigDeclId},
        library::{LibraryDecl, LibraryDeclId},
        udp::{UdpDecl, UdpDeclId},
    },
    has_source::HasSource,
    module::{
        clocking::{ClockingBlockDef, ClockingBlockId},
        generate::{GenerateRegion, GenerateRegionId},
        instantiation::{Instance, InstanceId, Instantiation, InstantiationId},
        port::Ports,
        specify::{SpecifyBlock, SpecifyBlockId, SpecifyBlockItem},
    },
    owner::OwnerId,
    proc::{Proc, ProcId},
    region_tree::{RegionNode, RegionTreeIterator},
    source_map::{HirLookup, Lowered, NamedSourceLookup, SourceInfo, SourceLookup},
    stmt::{CaseItem, ForInit, StmtId, StmtKind},
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
    if db.file_kind(file_id).is_project_manifest() {
        return crate::manifest::document_symbols(db, file_id);
    }
    let file_id = HirFileId::File(file_id);
    let lowered =
        db.body_with_source_map(db.owner_table(file_id).file_owner().expect("file owner"));
    let body = db.body_with_source_map(db.owner_table(file_id).file_owner().expect("file owner"));
    let file = lowered.data_ref();
    let src_map = lowered.source_map();
    let owner = db.owner_table(file_id).file_owner().expect("file owner must exist");
    let region_tree = db.owner_region_tree(owner);
    let mut regions = region_tree.walk().peekable();
    let projection = db.source_projection(file_id);

    let mut collector =
        SymbolCollector::new(file.items.len() + region_tree.root_count() + body.decls.len());

    for item in &file.items {
        if let Some(range) = src_map
            .item_to_source(db, item)
            .and_then(|source| projection.origin(source))
            .and_then(|origin| origin.full_range())
        {
            regions.add_region_symbol(range, &mut collector);
        }

        match item.clone() {
            BodyItem::ModuleOwner(owner) => {
                collect_module_items(db, owner, &mut collector);
            }
            BodyItem::AnonymousProgramOwner(owner) => {
                collect_module_items(db, owner, &mut collector);
            }
            BodyItem::ProcId(proc_id) => {
                let proc = lowered.get(proc_id);
                let body = db.body_with_source_map(proc.owner);
                if let Some(stmt_id) = body.root_stmt {
                    build_stmt(db, &mut collector, stmt_id, body.as_ref());
                }
            }
            BodyItem::DeclarationId(declaration_id) => {
                build_declaration(db, &mut collector, declaration_id, body.as_ref());
            }
            BodyItem::TypedefId(typedef_id) => {
                build_typedef(db, &mut collector, typedef_id, body.as_ref())
            }
            BodyItem::SubroutineOwner(owner) => build_subroutine(db, &mut collector, owner),
            BodyItem::StructId(struct_id) => {
                build_struct(db, &mut collector, struct_id, body.as_ref())
            }
            BodyItem::ConfigDeclId(config_id) => {
                build_config_decl(db, &mut collector, config_id, lowered.as_ref())
            }
            BodyItem::LibraryDeclId(library_id) => {
                build_library_decl(db, &mut collector, library_id, lowered.as_ref())
            }
            BodyItem::LibraryIncludeId(_) => {}
            BodyItem::AssertionStmtId(_) => {}
            BodyItem::ClassId(_)
            | BodyItem::BindDirectiveId(_)
            | BodyItem::ConstraintDefId(_)
            | BodyItem::TimeUnitsDeclId(_)
            | BodyItem::DpiImportId(_)
            | BodyItem::DpiExportId(_)
            | BodyItem::ExternInterfaceMethodId(_)
            | BodyItem::ExternModuleDeclId(_)
            | BodyItem::ExternUdpDeclId(_)
            | BodyItem::NetTypeDeclId(_)
            | BodyItem::NetAliasId(_)
            | BodyItem::ElabSystemTaskId(_)
            | BodyItem::LetDeclId(_) => {}
            BodyItem::CheckerOwner(owner) => build_checker_owner(db, &mut collector, owner),
            BodyItem::CovergroupOwner(owner) => build_covergroup_owner(db, &mut collector, owner),
            BodyItem::PropertyId(property_id) => {
                build_property(db, &mut collector, property_id, body.as_ref())
            }
            BodyItem::SequenceId(sequence_id) => {
                build_sequence(db, &mut collector, sequence_id, body.as_ref())
            }
            BodyItem::UdpDeclId(udp_id) => {
                build_udp_decl(db, &mut collector, udp_id, lowered.as_ref())
            }
            invalid @ (BodyItem::ContAssignId(_)
            | BodyItem::DefParamId(_)
            | BodyItem::GenerateRegionId(_)
            | BodyItem::GenerateBlockOwner(_)
            | BodyItem::SpecifyBlockId(_)
            | BodyItem::SpecifyItemId(_)
            | BodyItem::InstantiationId(_)
            | BodyItem::PortDeclId(_)
            | BodyItem::ModportId(_)
            | BodyItem::ClockingBlockOwner(_)) => {
                panic!("file owner lowered a non-file item: {invalid:?}")
            }
        }
    }

    regions.finish_all(&mut collector);
    collector.finish()
}

fn collect_module_items(db: &dyn TyDb, owner: OwnerId, collector: &mut SymbolCollector) {
    let lowered = db.body_with_source_map(owner);
    let body = db.body_with_source_map(owner);
    let module = lowered.data_ref();
    let src_map = lowered.source_map();
    let region_tree = db.owner_region_tree(owner);
    let mut regions = region_tree.walk().peekable();
    let projection = db.source_projection(owner.file(db));

    let Some(InFile { value: module_src, .. }) = owner.source(db) else {
        return;
    };
    collector.push_symbol_with_children(
        &module.name,
        module_src,
        module.items.len() + body.decls.len() + body.stmts.len(),
    );

    if let Some(params) = &module.param_ports {
        for decl_id in params.clone() {
            if let Some(src) = body.source_info(db, decl_id) {
                regions.add_region_symbol(src.full_range(), collector);
            }
            build_decl(db, collector, decl_id, SymbolKind::ParamDecl, body.as_ref());
        }
    }

    match &module.ports {
        Ports::NonAnsi { ports, .. } => {
            for (port_id, port) in ports.iter() {
                if let Some(src) = lowered.named_source_info(db, port_id) {
                    regions.add_region_symbol(src.full_range(), collector);
                    collector.push_symbol(&port.label, src);
                    collector.pop();
                }
            }
        }
        Ports::Ansi(port_decls) => {
            for (port_id, port_decl) in port_decls.iter() {
                if let Some(src) = lowered.source_info(db, port_id) {
                    regions.add_region_symbol(src.full_range(), collector);
                }
                build_decls(db, collector, &port_decl.decls, SymbolKind::PortDecl, body.as_ref());
            }
        }
    }

    for item in &module.items {
        if let Some(range) = src_map
            .item_to_source(db, item)
            .and_then(|source| projection.origin(source))
            .and_then(|origin| origin.full_range())
        {
            regions.add_region_symbol(range, collector);
        }
        match item.clone() {
            BodyItem::DeclarationId(declaration_id) => {
                build_declaration(db, collector, declaration_id, body.as_ref())
            }
            BodyItem::InstantiationId(instantiation_id) => {
                for &instance_id in lowered.get(instantiation_id).instances.iter() {
                    let hir = lowered.get(instance_id);
                    if let Some(src) = lowered.named_source_info(db, instance_id) {
                        collector.push_symbol(&hir.name, src);
                        collector.pop();
                    }
                }
            }
            BodyItem::ProcId(proc_id) => {
                let proc = lowered.get(proc_id);
                let body = db.body_with_source_map(proc.owner);
                if let Some(stmt_id) = body.root_stmt {
                    build_stmt(db, collector, stmt_id, body.as_ref());
                }
            }
            BodyItem::PortDeclId(port_decl) => {
                let port_decl = lowered.get(port_decl);
                build_decls(db, collector, &port_decl.decls, SymbolKind::PortDecl, body.as_ref())
            }
            BodyItem::ContAssignId(_) => {}
            BodyItem::DefParamId(_) => {}
            BodyItem::GenerateRegionId(generate_region_id) => build_generate_region(
                db,
                collector,
                generate_region_id,
                lowered.as_ref(),
                body.as_ref(),
            ),
            BodyItem::SpecifyBlockId(specify_block_id) => build_specify_block(
                db,
                collector,
                specify_block_id,
                lowered.as_ref(),
                body.as_ref(),
            ),
            BodyItem::SpecifyItemId(_) => {}
            BodyItem::TypedefId(typedef_id) => {
                build_typedef(db, collector, typedef_id, body.as_ref())
            }
            BodyItem::SubroutineOwner(owner) => build_subroutine(db, collector, owner),
            BodyItem::ModportId(modport_id) => {
                let modport = lowered.get(modport_id);
                if let Some(src) = lowered.named_source_info(db, modport_id) {
                    collector.push_symbol(&modport.name, src);
                    collector.pop();
                }
            }
            BodyItem::ClockingBlockOwner(owner) => {
                build_clocking_block_owner(db, collector, owner);
            }
            BodyItem::AssertionStmtId(_) => {}
            BodyItem::ClassId(_)
            | BodyItem::BindDirectiveId(_)
            | BodyItem::ConstraintDefId(_)
            | BodyItem::TimeUnitsDeclId(_)
            | BodyItem::DpiImportId(_)
            | BodyItem::DpiExportId(_)
            | BodyItem::ExternInterfaceMethodId(_)
            | BodyItem::ExternModuleDeclId(_)
            | BodyItem::ExternUdpDeclId(_)
            | BodyItem::NetTypeDeclId(_)
            | BodyItem::NetAliasId(_)
            | BodyItem::ElabSystemTaskId(_)
            | BodyItem::LetDeclId(_) => {}
            BodyItem::CheckerOwner(owner) => {
                build_checker_owner(db, collector, owner);
            }
            BodyItem::CovergroupOwner(owner) => {
                build_covergroup_owner(db, collector, owner);
            }
            BodyItem::PropertyId(property_id) => {
                build_property(db, collector, property_id, body.as_ref());
            }
            BodyItem::SequenceId(sequence_id) => {
                build_sequence(db, collector, sequence_id, body.as_ref());
            }
            BodyItem::StructId(struct_id) => build_struct(db, collector, struct_id, body.as_ref()),
            invalid @ (BodyItem::ModuleOwner(_)
            | BodyItem::AnonymousProgramOwner(_)
            | BodyItem::ConfigDeclId(_)
            | BodyItem::UdpDeclId(_)
            | BodyItem::LibraryDeclId(_)
            | BodyItem::LibraryIncludeId(_)
            | BodyItem::GenerateBlockOwner(_)) => {
                panic!("module owner lowered a non-module item: {invalid:?}")
            }
        }
    }
    collector.pop();
    regions.finish_all(collector);
}

fn collect_block_items(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    owner: OwnerId,
    lowered: &Lowered<Body>,
) {
    let body = lowered.data_ref();
    let scope = body.scope(owner).expect("lowered body must contain its block scope");
    let src_map = lowered.source_map();
    let region_tree = db.owner_region_tree(owner);
    let mut regions = region_tree.walk().peekable();
    let projection = db.source_projection(owner.file(db));

    let Some(InFile { value: block_src, .. }) = owner.source(db) else {
        return;
    };
    collector.push_symbol_with_children(
        &owner.name(db),
        block_src,
        scope.declarators().len() + scope.items().len(),
    );

    for item in scope.items() {
        if let Some(range) = src_map
            .block_item_to_source(item)
            .and_then(|source| projection.origin(source))
            .and_then(|origin| origin.full_range())
        {
            regions.add_region_symbol(range, collector);
        }
        match *item {
            BlockItem::DeclarationId(declaration_id) => {
                build_declaration(db, collector, declaration_id, lowered)
            }
            BlockItem::StmtId(stmt_id) => build_stmt(db, collector, stmt_id, lowered),
            BlockItem::TypedefId(typedef_id) => build_typedef(db, collector, typedef_id, lowered),
            BlockItem::StructId(struct_id) => build_struct(db, collector, struct_id, lowered),
        }
    }
    collector.pop();
    regions.finish_all(collector);
}
fn build_stmt(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    stmt_id: StmtId,
    lowered: &Lowered<Body>,
) {
    let stmt = lowered.get(stmt_id);

    if let StmtKind::Block(owner) = stmt.kind {
        collect_block_items(db, collector, owner, lowered);
        return;
    }

    let Some(stmt_src) = lowered.named_source_info(db, stmt_id) else {
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
        StmtKind::RandCase { items } => {
            for item in items {
                build_stmt(db, collector, item.clause, lowered);
            }
        }
        StmtKind::For { inits, stmt, .. } => {
            if let ForInit::Init(inits) = inits {
                for (_, decl_id) in inits {
                    build_decl(db, collector, *decl_id, SymbolKind::DataDecl, lowered);
                }
            }
            build_stmt(db, collector, *stmt, lowered);
        }
        StmtKind::Foreach { stmt, .. } => build_stmt(db, collector, *stmt, lowered),
        StmtKind::WaitOrder { action, else_stmt, .. } => {
            build_stmt(db, collector, *action, lowered);
            if let Some(else_stmt) = else_stmt {
                build_stmt(db, collector, *else_stmt, lowered);
            }
        }
        StmtKind::ImmediateAssertion { action, .. }
        | StmtKind::ConcurrentAssertion { action, .. } => {
            if let Some(pass) = action.pass {
                build_stmt(db, collector, pass, lowered);
            }
            if let Some(fail) = action.fail {
                build_stmt(db, collector, fail, lowered);
            }
        }

        StmtKind::Missing
        | StmtKind::Invalid
        | StmtKind::Unsupported(_)
        | StmtKind::Empty
        | StmtKind::Expr(_)
        | StmtKind::VoidCastedCall(_)
        | StmtKind::Jump(_)
        | StmtKind::EventTrigger(_)
        | StmtKind::ProcAssign(_)
        | StmtKind::CheckerInstance(_)
        | StmtKind::WaitFork
        | StmtKind::DisableFork
        | StmtKind::Disable(_)
        | StmtKind::Block(_) => {}
    }
    collector.pop();
}

fn build_declaration<L>(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    declaration_id: DeclarationId,
    lowered: &L,
) where
    L: HirLookup<DeclId, Hir = Declarator>
        + HirLookup<DeclarationId, Hir = Declaration>
        + NamedSourceLookup<DeclId>
        + SourceLookup<DeclarationId>,
{
    let declaration = lowered.hir(declaration_id);
    let Some(src) = lowered.source_info(db, declaration_id) else {
        return;
    };
    build_decls(
        db,
        collector,
        &declaration.decls(),
        SymbolKind::from_syntax_kind(
            src.kind().expect("mapped source should retain its syntax kind"),
        ),
        lowered,
    );
}

#[inline]
fn build_generate_region<S>(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    generate_region_id: GenerateRegionId,
    structure: &S,
    body: &Lowered<Body>,
) where
    S: HirLookup<GenerateRegionId, Hir = GenerateRegion>
        + HirLookup<InstanceId, Hir = Instance>
        + HirLookup<InstantiationId, Hir = Instantiation>
        + HirLookup<ProcId, Hir = Proc>
        + NamedSourceLookup<GenerateRegionId>
        + NamedSourceLookup<InstanceId>,
{
    let hir = structure.hir(generate_region_id);
    let Some(src) = structure.named_source_info(db, generate_region_id) else {
        return;
    };
    let name = Some(SmolStr::new_static("generate"));
    collector.push_symbol_with_kind(&name, src, SymbolKind::Generate);
    for item in hir.items.iter() {
        build_generate_block_item(db, collector, item.clone(), structure, body);
    }
    collector.pop();
}

fn build_generate_block(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    generate_block_owner: OwnerId,
) {
    let owner = generate_block_owner;
    let Some(InFile { value: generate_block_src, .. }) = owner.source(db) else {
        return;
    };
    let lowered = db.body_with_source_map(owner);
    let body = db.body_with_source_map(owner);
    let generate_block = lowered.data_ref();
    let name = generate_block.name.clone();

    collector.push_symbol_with_kind(&name, generate_block_src, SymbolKind::Generate);
    for item in &generate_block.items {
        build_generate_block_item(db, collector, item.clone(), lowered.as_ref(), body.as_ref());
    }
    collector.pop();
}

/// Builds a document symbol for one generate item, shared by generate regions
/// (whose items live in the enclosing module's arenas) and generate blocks
/// (whose items live in their own container).
#[inline]
fn build_generate_block_item<S>(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    item: BodyItem,
    structure: &S,
    body: &Lowered<Body>,
) where
    S: HirLookup<InstanceId, Hir = Instance>
        + HirLookup<InstantiationId, Hir = Instantiation>
        + HirLookup<ProcId, Hir = Proc>
        + NamedSourceLookup<InstanceId>,
{
    match item {
        BodyItem::ContAssignId(_) | BodyItem::DefParamId(_) => {}
        BodyItem::DeclarationId(declaration_id) => {
            build_declaration(db, collector, declaration_id, body);
        }
        BodyItem::GenerateBlockOwner(child_id) => {
            build_generate_block(db, collector, child_id);
        }
        BodyItem::TypedefId(typedef_id) => {
            build_typedef(db, collector, typedef_id, body);
        }
        BodyItem::SubroutineOwner(owner) => {
            build_subroutine(db, collector, owner);
        }
        BodyItem::ProcId(proc_id) => {
            let proc = structure.hir(proc_id);
            let body = db.body_with_source_map(proc.owner);
            if let Some(stmt_id) = body.root_stmt {
                build_stmt(db, collector, stmt_id, body.as_ref());
            }
        }
        BodyItem::InstantiationId(instantiation_id) => {
            for &instance_id in structure.hir(instantiation_id).instances.iter() {
                let hir = structure.hir(instance_id);
                if let Some(src) = structure.named_source_info(db, instance_id) {
                    collector.push_symbol(&hir.name, src);
                    collector.pop();
                }
            }
        }
        BodyItem::StructId(struct_id) => {
            build_struct(db, collector, struct_id, body);
        }
        BodyItem::AssertionStmtId(_) => {}
        BodyItem::ClassId(_)
        | BodyItem::BindDirectiveId(_)
        | BodyItem::ConstraintDefId(_)
        | BodyItem::TimeUnitsDeclId(_)
        | BodyItem::DpiImportId(_)
        | BodyItem::DpiExportId(_)
        | BodyItem::ExternInterfaceMethodId(_)
        | BodyItem::ExternModuleDeclId(_)
        | BodyItem::ExternUdpDeclId(_)
        | BodyItem::NetTypeDeclId(_)
        | BodyItem::NetAliasId(_)
        | BodyItem::ElabSystemTaskId(_)
        | BodyItem::LetDeclId(_) => {}
        invalid @ (BodyItem::ModuleOwner(_)
        | BodyItem::AnonymousProgramOwner(_)
        | BodyItem::ConfigDeclId(_)
        | BodyItem::UdpDeclId(_)
        | BodyItem::LibraryDeclId(_)
        | BodyItem::LibraryIncludeId(_)
        | BodyItem::CheckerOwner(_)
        | BodyItem::CovergroupOwner(_)
        | BodyItem::PropertyId(_)
        | BodyItem::SequenceId(_)
        | BodyItem::SpecifyBlockId(_)
        | BodyItem::SpecifyItemId(_)
        | BodyItem::GenerateRegionId(_)
        | BodyItem::PortDeclId(_)
        | BodyItem::ModportId(_)
        | BodyItem::ClockingBlockOwner(_)) => {
            panic!("generate owner lowered a non-generate item: {invalid:?}")
        }
    }
}
fn build_checker_owner(db: &dyn TyDb, collector: &mut SymbolCollector, owner: OwnerId) {
    let Some(checker) = owner.as_checker(db) else {
        return;
    };
    let body = db.body_with_source_map(owner);
    build_checker(db, collector, checker.value, body.as_ref());
}

fn build_clocking_block_owner(db: &dyn TyDb, collector: &mut SymbolCollector, owner: OwnerId) {
    let Some(clocking_block) = owner.as_clocking_block(db) else {
        return;
    };
    let body = db.body_with_source_map(owner);
    build_clocking_block(db, collector, clocking_block.value, body.as_ref());
}

fn build_covergroup_owner(db: &dyn TyDb, collector: &mut SymbolCollector, owner: OwnerId) {
    let Some(covergroup) = owner.as_covergroup(db) else {
        return;
    };
    let body = db.body_with_source_map(owner);
    build_covergroup(db, collector, covergroup.value, body.as_ref());
}

fn build_checker<L>(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    checker_id: CheckerId,
    lowered: &L,
) where
    L: HirLookup<CheckerId, Hir = CheckerDef> + NamedSourceLookup<CheckerId>,
{
    let checker = lowered.hir(checker_id);
    let Some(src) = lowered.named_source_info(db, checker_id) else {
        return;
    };
    collector.push_symbol(&checker.name, src);
    collector.pop();
}
#[inline]
fn build_property<L>(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    property_id: PropertyId,
    lowered: &L,
) where
    L: HirLookup<PropertyId, Hir = PropertyDef> + NamedSourceLookup<PropertyId>,
{
    let property = lowered.hir(property_id);
    if let Some(src) = lowered.named_source_info(db, property_id) {
        collector.push_symbol(&property.name, src);
        collector.pop();
    }
}

#[inline]
fn build_sequence<L>(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    sequence_id: SequenceId,
    lowered: &L,
) where
    L: HirLookup<SequenceId, Hir = SequenceDef> + NamedSourceLookup<SequenceId>,
{
    let sequence = lowered.hir(sequence_id);
    if let Some(src) = lowered.named_source_info(db, sequence_id) {
        collector.push_symbol(&sequence.name, src);
        collector.pop();
    }
}

#[inline]
fn build_clocking_block<L>(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    clocking_block_id: ClockingBlockId,
    lowered: &L,
) where
    L: HirLookup<ClockingBlockId, Hir = ClockingBlockDef> + NamedSourceLookup<ClockingBlockId>,
{
    let clocking_block = lowered.hir(clocking_block_id);
    let Some(src) = lowered.named_source_info(db, clocking_block_id) else {
        return;
    };
    collector.push_symbol(&clocking_block.name, src);
    collector.pop();
}

#[inline]
fn build_covergroup<L>(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    covergroup_id: CovergroupId,
    lowered: &L,
) where
    L: HirLookup<CovergroupId, Hir = CovergroupDef>
        + HirLookup<CoverpointId, Hir = CoverpointDef>
        + HirLookup<CrossId, Hir = CrossDef>
        + NamedSourceLookup<CovergroupId>
        + NamedSourceLookup<CoverpointId>
        + NamedSourceLookup<CrossId>,
{
    let covergroup = lowered.hir(covergroup_id);
    let Some(src) = lowered.named_source_info(db, covergroup_id) else {
        return;
    };
    collector.push_symbol_with_children(
        &covergroup.name,
        src,
        covergroup.coverpoints.len() + covergroup.crosses.len(),
    );
    for &coverpoint_id in &covergroup.coverpoints {
        build_coverpoint(db, collector, coverpoint_id, lowered);
    }
    for &cross_id in &covergroup.crosses {
        build_cross(db, collector, cross_id, lowered);
    }
    collector.pop();
}

#[inline]
fn build_coverpoint<L>(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    coverpoint_id: CoverpointId,
    lowered: &L,
) where
    L: HirLookup<CoverpointId, Hir = CoverpointDef> + NamedSourceLookup<CoverpointId>,
{
    let coverpoint = lowered.hir(coverpoint_id);
    let Some(src) = lowered.named_source_info(db, coverpoint_id) else {
        return;
    };
    collector.push_symbol(&coverpoint.name, src);
    collector.pop();
}

#[inline]
fn build_cross<L>(db: &dyn TyDb, collector: &mut SymbolCollector, cross_id: CrossId, lowered: &L)
where
    L: HirLookup<CrossId, Hir = CrossDef> + NamedSourceLookup<CrossId>,
{
    let cross = lowered.hir(cross_id);
    let Some(src) = lowered.named_source_info(db, cross_id) else {
        return;
    };
    collector.push_symbol(&cross.name, src);
    collector.pop();
}

#[inline]
fn build_struct<L>(db: &dyn TyDb, collector: &mut SymbolCollector, struct_id: StructId, lowered: &L)
where
    L: HirLookup<StructId, Hir = StructDef> + NamedSourceLookup<StructId>,
{
    let hir = lowered.hir(struct_id);
    let Some(src) = lowered.named_source_info(db, struct_id) else {
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
fn build_specify_block<S>(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    specify_block_id: SpecifyBlockId,
    structure: &S,
    body: &Lowered<Body>,
) where
    S: HirLookup<SpecifyBlockId, Hir = SpecifyBlock> + NamedSourceLookup<SpecifyBlockId>,
{
    let hir = structure.hir(specify_block_id);
    let Some(src) = structure.named_source_info(db, specify_block_id) else {
        return;
    };
    let name = Some(SmolStr::new_static("specify"));
    collector.push_symbol_with_kind(&name, src, SymbolKind::Specify);
    for item in hir.items.iter() {
        match *item {
            SpecifyBlockItem::DeclarationId(declaration_id) => {
                build_declaration(db, collector, declaration_id, body);
            }
            SpecifyBlockItem::SpecifyItemId(_) => {}
        }
    }
    collector.pop();
}

#[inline]
fn build_decls<L>(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    decls: &DeclsRange,
    kind: SymbolKind,
    lowered: &L,
) where
    L: HirLookup<DeclId, Hir = Declarator> + NamedSourceLookup<DeclId>,
{
    for decl in decls.clone() {
        build_decl(db, collector, decl, kind, lowered);
    }
}

#[inline]
fn build_decl<L>(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    decl: DeclId,
    kind: SymbolKind,
    lowered: &L,
) where
    L: HirLookup<DeclId, Hir = Declarator> + NamedSourceLookup<DeclId>,
{
    let hir = lowered.hir(decl);
    let Some(src) = lowered.named_source_info(db, decl) else {
        return;
    };
    collector.push_symbol_with_kind(&hir.name, src, kind);
    collector.pop();
}

#[inline]
fn build_typedef<L>(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    typedef_id: TypedefId,
    lowered: &L,
) where
    L: HirLookup<TypedefId, Hir = Typedef> + NamedSourceLookup<TypedefId>,
{
    let hir = lowered.hir(typedef_id);
    let Some(src) = lowered.named_source_info(db, typedef_id) else {
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
fn build_subroutine(db: &dyn TyDb, collector: &mut SymbolCollector, owner: OwnerId) {
    let hir = db.subroutine(owner);
    let Some(src) = owner.source(db).map(|source| source.value) else {
        return;
    };
    collector.push_symbol_with_kind(&hir.name, src, SymbolKind::Fn);
    collector.pop();
}

#[inline]
fn build_config_decl<L>(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    config_id: ConfigDeclId,
    lowered: &L,
) where
    L: HirLookup<ConfigDeclId, Hir = ConfigDecl> + NamedSourceLookup<ConfigDeclId>,
{
    let hir = lowered.hir(config_id);
    let Some(src) = lowered.named_source_info(db, config_id) else {
        return;
    };
    collector.push_symbol_with_kind(&hir.name, src, SymbolKind::Config);
    collector.pop();
}

#[inline]
fn build_udp_decl<L>(db: &dyn TyDb, collector: &mut SymbolCollector, udp_id: UdpDeclId, lowered: &L)
where
    L: HirLookup<UdpDeclId, Hir = UdpDecl> + NamedSourceLookup<UdpDeclId>,
{
    let hir = lowered.hir(udp_id);
    let Some(src) = lowered.named_source_info(db, udp_id) else {
        return;
    };
    collector.push_symbol_with_kind(&hir.name, src, SymbolKind::Primitive);
    collector.pop();
}

#[inline]
fn build_library_decl<L>(
    db: &dyn TyDb,
    collector: &mut SymbolCollector,
    library_id: LibraryDeclId,
    lowered: &L,
) where
    L: HirLookup<LibraryDeclId, Hir = LibraryDecl> + NamedSourceLookup<LibraryDeclId>,
{
    let hir = lowered.hir(library_id);
    let Some(src) = lowered.named_source_info(db, library_id) else {
        return;
    };
    collector.push_symbol_with_kind(&hir.name, src, SymbolKind::Library);
    collector.pop();
}
