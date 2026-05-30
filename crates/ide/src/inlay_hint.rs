use hir::{
    container::InFile,
    db::HirDb,
    file::HirFileId,
    hir_def::{
        Ident,
        expr::{
            Expr,
            declarator::{DeclId, DeclaratorParent},
        },
        file::FileItem,
        module::{
            Module, ModuleId, ModuleSourceMap, ModuleSrc,
            instantiation::{Instantiation, ParamAssign, PortConn, PortConnId},
            port::{NonAnsiPortId, PortDeclId, PortDirection, Ports},
        },
    },
    scope::{AnsiPortEntry, ModuleEntry, ModuleScope, NonAnsiPortEntry},
    source_map::{IsNamedSrc, IsSrc},
};
use syntax::{ast, match_ast_kind};
use utils::{
    check_or_throw,
    get::{Get, GetRef},
    text_edit::{TextEdit, TextRange, TextSize},
};
use vfs::FileId;

use crate::{db::root_db::RootDb, markup::Markup, module_resolution::resolve_module_name};

#[derive(Debug)]
pub struct InlayHintConfig {
    pub port_connection: bool,
    pub parameter_assignment: bool,
    pub end_structure: bool,
}

impl InlayHintConfig {
    fn instantiation(&self) -> bool {
        self.port_connection || self.parameter_assignment
    }
}

#[derive(Debug, Copy, Clone, Hash)]
pub enum InlayKind {
    ParamAssign,
    Port,
    EndStructure,
}

#[derive(Debug)]
pub struct InlayHint {
    pub label: String,
    pub tooltip: Option<Markup>,
    pub target_location: Option<InFile<TextRange>>,
    pub padding_left: bool,
    pub padding_right: bool,

    pub position: TextSize,
    pub kind: InlayKind,
    pub text_edit: Option<TextEdit>,
}

#[derive(Debug, Copy, Clone)]
struct HintAnchor {
    range: TextRange,
    position: TextSize,
    kind: InlayKind,
    padding_left: bool,
    padding_right: bool,
}

impl HintAnchor {
    fn from_src(src: impl IsSrc, position: Option<TextSize>) -> Option<Self> {
        let range = src.range();
        let kind = match_ast_kind! { src.kind(),
            ast::ParamAssignment => InlayKind::ParamAssign,
            ast::OrderedPortConnection | ast::EmptyPortConnection | ast::NamedPortConnection => InlayKind::Port,
            _ => return None,
        };
        let (padding_left, padding_right) = match_ast_kind! { src.kind(),
            ast::ParamAssignment => (false, true),
            ast::OrderedPortConnection | ast::EmptyPortConnection => (false, true),
            ast::NamedPortConnection => (true, true),
            _ => (false, false),
        };

        Some(Self {
            range,
            position: position.unwrap_or_else(|| range.start()),
            kind,
            padding_left,
            padding_right,
        })
    }

    fn module_end(range: TextRange) -> Self {
        Self {
            range,
            position: range.end(),
            kind: InlayKind::EndStructure,
            padding_left: true,
            padding_right: false,
        }
    }
}

struct InlayHintCollector {
    hints: Vec<InlayHint>,
    range: TextRange,
    config: InlayHintConfig,
}

impl InlayHintCollector {
    fn new(range: TextRange, config: InlayHintConfig) -> Self {
        Self { hints: Vec::new(), range, config }
    }

    fn collect_hint(
        &mut self,
        anchor: HintAnchor,
        target_src: Option<InFile<impl IsSrc>>,
        label: String,
        text_edit: Option<TextEdit>,
    ) {
        if !self.intersect(anchor.range) {
            return;
        }

        let (tooltip, target_location) = if let Some(InFile { value: src, file_id }) = target_src {
            let location = InFile::new(file_id, src.range());
            (Some(Markup::new()), Some(location))
        } else {
            (None, None)
        };

        self.hints.push(InlayHint {
            label,
            tooltip,
            target_location,
            padding_left: anchor.padding_left,
            padding_right: anchor.padding_right,
            position: anchor.position,
            kind: anchor.kind,
            text_edit,
        });
    }

    fn collect_src_hint(
        &mut self,
        src: impl IsSrc,
        target_src: Option<InFile<impl IsSrc>>,
        position: Option<TextSize>,
        label: String,
        text_edit: Option<TextEdit>,
    ) {
        if let Some(anchor) = HintAnchor::from_src(src, position) {
            self.collect_hint(anchor, target_src, label, text_edit);
        }
    }

    fn collect_module_end_hint(&mut self, module_src: ModuleSrc, name: &str) {
        if let Some(end_range) = module_src.end_range() {
            self.collect_hint(
                HintAnchor::module_end(end_range),
                None::<InFile<ModuleSrc>>,
                format!(": {name}"),
                None,
            );
        }
    }

    fn into_hints(self) -> Vec<InlayHint> {
        self.hints
    }

    fn intersect(&self, range: TextRange) -> bool {
        self.range.intersect(range).is_some()
    }
}

pub(crate) fn inlay_hint(
    db: &RootDb,
    file_id: FileId,
    range: TextRange,
    config: InlayHintConfig,
) -> Vec<InlayHint> {
    let file_id = HirFileId(file_id);
    let (file, src_map) = db.hir_file_with_source_map(file_id);
    let (_file, src_map) = (file.as_ref(), src_map.as_ref());

    let mut collector = InlayHintCollector::new(range, config);

    for &item in src_map.items.iter() {
        #[allow(clippy::single_match)]
        match item {
            FileItem::LocalModuleId(idx) => {
                let module_id = ModuleId::new(file_id, idx);
                let Some(module_src) = src_map.get(idx) else {
                    continue;
                };

                if collector.intersect(module_src.range()) {
                    collect_module_items(db, module_id, module_src, &mut collector);
                }
            }
            _ => {}
        }
    }

    collector.into_hints()
}

fn collect_module_items(
    db: &RootDb,
    module_id: ModuleId,
    module_src: ModuleSrc,
    collector: &mut InlayHintCollector,
) {
    let (module, src_map) = db.module_with_source_map(module_id);
    let (module, src_map) = (module.as_ref(), src_map.as_ref());

    if collector.config.instantiation() {
        for (instantiation_id, instantiation) in module.instantiations.iter() {
            let Some(instantiation_src) = src_map.get(instantiation_id) else {
                continue;
            };
            if collector.intersect(instantiation_src.range()) {
                process_instantiation(db, module_id, module, src_map, instantiation, collector);
            }
        }
    }

    if collector.config.end_structure
        && let Some(name) = &module.name
    {
        collector.collect_module_end_hint(module_src, name);
    }
}

fn process_instantiation(
    db: &RootDb,
    module_id: ModuleId,
    module: &Module,
    src_map: &ModuleSourceMap,
    instantiation: &Instantiation,
    collector: &mut InlayHintCollector,
) -> Option<()> {
    let target_module_id =
        resolve_module_name(db, module_id.file_id.file_id(), instantiation.module_name.as_ref()?)
            .unique()?;

    let target_file = target_module_id.file_id;
    let (target_module, target_src_map) = db.module_with_source_map(target_module_id);
    let (target_module, target_src_map) = (target_module.as_ref(), target_src_map.as_ref());
    let target_scope = db.module_scope(target_module_id);
    let target_scope = target_scope.as_ref();

    // handle param assignments
    if collector.config.parameter_assignment {
        for (id, &assign_id) in instantiation.param_assigns.iter().enumerate() {
            try {
                let ParamAssign::Ordered(assign_expr) = module.get(assign_id) else {
                    continue;
                };
                let assign_src = src_map.get(assign_id)?;
                check_or_throw!(collector.intersect(assign_src.range()));

                let param_id = target_module.param_port_id_by_idx(id)?;
                let param_name = target_module.get(param_id).name.as_ref()?;
                check_or_throw!(!should_skip(module.get(*assign_expr), param_name));
                let target_src = InFile::new(target_file, target_src_map.get(param_id)?);
                collector.collect_src_hint(
                    assign_src,
                    Some(target_src),
                    None,
                    format!("{param_name}:"),
                    edits_for_conn(param_name, assign_src),
                );
            };
        }
    }

    // handle port connections
    if collector.config.port_connection {
        for instance_id in instantiation.instances.iter() {
            let instance = module.get(*instance_id);
            let Some(instance_src) = src_map.get(*instance_id) else {
                continue;
            };
            if !collector.intersect(instance_src.range()) {
                continue;
            }

            for (idx, &conn_id) in instance.connections.iter().enumerate() {
                try {
                    let conn = module.get(conn_id);
                    let conn_src = src_map.get(conn_id)?;
                    check_or_throw!(collector.intersect(conn_src.range()));

                    match &target_module.ports {
                        Ports::NonAnsi { .. } => {
                            let (port_id, name, dir) =
                                non_ansi_port_id_for_conn(target_module, target_scope, conn, idx)?;
                            let target_src = InFile::new(target_file, target_src_map.get(port_id)?);
                            collect_connection_hint(
                                module, src_map, conn_id, name, dir, target_src, collector,
                            );
                        }
                        Ports::Ansi(_) => {
                            let (port_decl_id, decl_id) =
                                ansi_port_decl_id_for_conn(target_module, target_scope, conn, idx)?;
                            let port_decl = target_module.get(port_decl_id);
                            let name = target_module.get(decl_id).name.as_ref()?;
                            let dir = port_decl.header.dir();
                            let target_src = InFile::new(target_file, target_src_map.get(decl_id)?);
                            collect_connection_hint(
                                module, src_map, conn_id, name, dir, target_src, collector,
                            );
                        }
                    }
                };
            }
        }
    }

    Some(())
}

fn collect_connection_hint(
    module: &Module,
    src_map: &ModuleSourceMap,
    conn_id: PortConnId,
    name: &str,
    port_dir: PortDirection,
    target_src: InFile<impl IsSrc>,
    collector: &mut InlayHintCollector,
) -> Option<()> {
    let conn = module.get(conn_id);
    let conn_src = src_map.get(conn_id)?;
    let arrow = match port_dir {
        PortDirection::Input => "←",
        PortDirection::Output => "→",
        PortDirection::Inout => "↔",
        PortDirection::Ref => "&",
    };

    let conn_start = conn_src.range().start();
    match conn {
        PortConn::Empty => {
            let label = format!("{name} {arrow}");
            let edit = edits_for_conn(name, conn_src);
            collector.collect_src_hint(conn_src, Some(target_src), None, label, edit);
        }
        PortConn::Ordered(expr) => {
            let same_name = should_skip(module.get(*expr), name);
            let label = if same_name { arrow.to_string() } else { format!("{name} {arrow}") };
            let target_src = if same_name { None } else { Some(target_src) };
            let edit = if same_name { None } else { edits_for_conn(name, conn_src) };
            let position = src_map.get(*expr).map_or_else(|| conn_start, |src| src.range().start());
            collector.collect_src_hint(conn_src, target_src, Some(position), label, edit);
        }
        PortConn::Named(port_name, expr) => {
            let (label, target_src) =
                if port_name.as_ref().is_none_or(|port_name| port_name != name) {
                    (format!("{name} {arrow}"), Some(target_src))
                } else {
                    (arrow.to_string(), None)
                };
            let position = expr
                .and_then(|expr| src_map.get(expr).map(|src| src.range().start()))
                .or_else(|| conn_src.name_range().map(|range| range.start()))
                .unwrap_or(conn_start);
            collector.collect_src_hint(conn_src, target_src, Some(position), label, None);
        }
        PortConn::Wildcard => {}
    }

    Some(())
}

fn non_ansi_port_id_for_conn<'a>(
    module: &'a Module,
    scope: &ModuleScope,
    conn: &'a PortConn,
    idx: usize,
) -> Option<(NonAnsiPortId, &'a Ident, PortDirection)> {
    match conn {
        PortConn::Empty | PortConn::Ordered(_) => {
            let Ports::NonAnsi { ports, .. } = &module.ports else {
                return None;
            };
            let (port_id, port) = ports.iter().nth(idx)?;
            let name = port.label.as_ref()?;
            let dir = non_ansi_port_dir_by_port_id(module, scope, port_id)?;
            Some((port_id, name, dir))
        }
        PortConn::Named(Some(name), _) => {
            let ModuleEntry::NonAnsiPortEntry(NonAnsiPortEntry { label, .. }) = scope.get(name)?
            else {
                return None;
            };
            let port_id = label?;
            let port_name = module.get(port_id).label.as_ref()?;
            let dir = non_ansi_port_dir_by_port_id(module, scope, port_id)?;
            Some((port_id, port_name, dir))
        }
        PortConn::Named(None, _) | PortConn::Wildcard => None,
    }
}

fn non_ansi_port_dir_by_port_id(
    module: &Module,
    scope: &ModuleScope,
    port_id: NonAnsiPortId,
) -> Option<PortDirection> {
    let port = module.get(port_id);

    if let Some(refs) = port.refs.clone() {
        for ref_id in refs {
            let Some(name) = module.get(ref_id).ident.as_ref() else {
                continue;
            };
            if let Some(port_decl_id) = scope.non_ansi_port_decl_id_by_name(module, name) {
                return Some(module.get(port_decl_id).header.dir());
            }
        }
    }

    let name = port.label.as_ref()?;
    let port_decl_id = scope.non_ansi_port_decl_id_by_name(module, name)?;
    Some(module.get(port_decl_id).header.dir())
}

fn ansi_port_decl_id_for_conn(
    module: &Module,
    scope: &ModuleScope,
    conn: &PortConn,
    idx: usize,
) -> Option<(PortDeclId, DeclId)> {
    match conn {
        PortConn::Empty | PortConn::Ordered(_) => {
            let port_decl_id = module.ansi_port_decl_id_by_idx(idx)?;
            let decl_id = module.get(port_decl_id).decls.clone().next()?;
            Some((port_decl_id, decl_id))
        }
        PortConn::Named(Some(name), _) => {
            let ModuleEntry::AnsiPortEntry(AnsiPortEntry(decl_id)) = scope.get(name)? else {
                return None;
            };
            let DeclaratorParent::PortDeclId(port_decl_id) = module.get(decl_id).parent else {
                return None;
            };
            Some((port_decl_id, decl_id))
        }
        PortConn::Named(None, _) | PortConn::Wildcard => None,
    }
}

fn edits_for_conn(param: &str, conn_src: impl IsSrc) -> Option<TextEdit> {
    let mut builder = TextEdit::builder();
    builder.insert(conn_src.range().start(), format!(".{}(", param));
    builder.insert(conn_src.range().end(), String::from(")"));
    Some(builder.finish())
}

fn should_skip(expr: &Expr, name: &str) -> bool {
    // TODO: handle more cases
    #[allow(clippy::match_like_matches_macro)]
    match expr {
        Expr::Ident(ident) if ident == name => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use hir::base_db::{change::Change, source_root::SourceRoot};
    use triomphe::Arc;
    use utils::{
        lines::LineEnding,
        text_edit::{TextRange, TextSize},
    };
    use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

    use super::{InlayHintConfig, InlayKind, inlay_hint};
    use crate::db::root_db::RootDb;

    fn db_with_file(text: &str) -> (RootDb, FileId) {
        let file_id = FileId(0);
        let path = VfsPath::new_virtual_path("/test.sv".to_owned());

        let mut file_set = FileSet::default();
        file_set.insert(file_id, path);
        let root = SourceRoot::new_local(file_set);

        let mut change = Change::new();
        change.set_roots(vec![root]);
        change.add_changed_file(ChangedFile {
            file_id,
            change_kind: ChangeKind::Create(Arc::from(text), LineEnding::Unix),
        });

        let mut db = RootDb::new(None);
        change.apply(&mut db);
        (db, file_id)
    }

    fn port_config() -> InlayHintConfig {
        InlayHintConfig { port_connection: true, parameter_assignment: false, end_structure: false }
    }

    fn parameter_config() -> InlayHintConfig {
        InlayHintConfig { port_connection: false, parameter_assignment: true, end_structure: false }
    }

    fn end_structure_config() -> InlayHintConfig {
        InlayHintConfig { port_connection: false, parameter_assignment: false, end_structure: true }
    }

    fn port_hint_labels(text: &str) -> Vec<String> {
        let (db, file_id) = db_with_file(text);
        let range = TextRange::new(TextSize::from(0), TextSize::of(text));
        inlay_hint(&db, file_id, range, port_config())
            .into_iter()
            .filter(|hint| matches!(hint.kind, InlayKind::Port))
            .map(|hint| hint.label)
            .collect()
    }

    fn port_hint_labels_in_range(text: &str, range: TextRange) -> Vec<String> {
        let (db, file_id) = db_with_file(text);
        inlay_hint(&db, file_id, range, port_config())
            .into_iter()
            .filter(|hint| matches!(hint.kind, InlayKind::Port))
            .map(|hint| hint.label)
            .collect()
    }

    fn port_hints(text: &str) -> Vec<super::InlayHint> {
        let (db, file_id) = db_with_file(text);
        let range = TextRange::new(TextSize::from(0), TextSize::of(text));
        inlay_hint(&db, file_id, range, port_config())
            .into_iter()
            .filter(|hint| matches!(hint.kind, InlayKind::Port))
            .collect()
    }

    fn param_hint_labels(text: &str) -> Vec<String> {
        let (db, file_id) = db_with_file(text);
        let range = TextRange::new(TextSize::from(0), TextSize::of(text));
        inlay_hint(&db, file_id, range, parameter_config())
            .into_iter()
            .filter(|hint| matches!(hint.kind, InlayKind::ParamAssign))
            .map(|hint| hint.label)
            .collect()
    }

    #[test]
    fn comment_only_range_skips_module_end_hint() {
        let text = "\
module top;
    // ISSUE_RANGE_START
    // This comment-only area contains no module ending.
    // ISSUE_RANGE_END

endmodule
";
        let start = text.find("// ISSUE_RANGE_START").unwrap();
        let end_marker = text.find("// ISSUE_RANGE_END").unwrap();
        let end = end_marker + text[end_marker..].find('\n').unwrap() + 1;
        let range = TextRange::new(TextSize::of(&text[..start]), TextSize::of(&text[..end]));
        let (db, file_id) = db_with_file(text);

        let hints = inlay_hint(&db, file_id, range, end_structure_config());

        assert!(hints.is_empty(), "comment-only range returned hints: {hints:?}");
    }

    #[test]
    fn module_end_range_returns_end_structure_hint() {
        let text = "module top;\nendmodule\n";
        let start = text.find("endmodule").unwrap();
        let end = start + "endmodule".len();
        let range = TextRange::new(TextSize::of(&text[..start]), TextSize::of(&text[..end]));
        let (db, file_id) = db_with_file(text);

        let labels = inlay_hint(&db, file_id, range, end_structure_config())
            .into_iter()
            .filter(|hint| matches!(hint.kind, InlayKind::EndStructure))
            .map(|hint| hint.label)
            .collect::<Vec<_>>();

        assert_eq!(labels, vec![": top"]);
    }

    #[test]
    fn extra_ordered_connections_do_not_invent_ansi_port_hints() {
        let text = "module child(input a); endmodule\nmodule top; child u(1'b0, 1'b1); endmodule\n";

        assert_eq!(port_hint_labels(text), vec!["a ←"]);
    }

    #[test]
    fn extra_ordered_connections_do_not_invent_non_ansi_port_hints() {
        let text =
            "module child(a); input a; endmodule\nmodule top; child u(1'b0, 1'b1); endmodule\n";

        assert_eq!(port_hint_labels(text), vec!["a ←"]);
    }

    #[test]
    fn port_hints_show_direction_arrows() {
        let text = "module child(input i, output o, inout io, ref r); endmodule\n\
            module top; logic a, b, c, d; child u(a, b, c, d); endmodule\n";

        assert_eq!(port_hint_labels(text), vec!["i ←", "o →", "io ↔", "r &"]);
    }

    #[test]
    fn ordered_port_hint_omits_same_name_but_keeps_direction() {
        let text = "module child(output instr_addr_o); endmodule\n\
            module top; logic instr_addr_o; child u(instr_addr_o); endmodule\n";

        assert_eq!(port_hint_labels(text), vec!["→"]);
    }

    #[test]
    fn ordered_same_name_arrow_hint_is_not_clickable() {
        let text = "module child(output instr_addr_o); endmodule\n\
            module top; logic instr_addr_o; child u(instr_addr_o); endmodule\n";

        let hints = port_hints(text);
        assert_eq!(hints.iter().map(|hint| hint.label.as_str()).collect::<Vec<_>>(), vec!["→"]);
        assert!(hints[0].target_location.is_none());
        assert!(hints[0].text_edit.is_none());
    }

    #[test]
    fn named_same_name_arrow_hint_is_not_clickable() {
        let text = "module child(output clk); endmodule\n\
            module top; logic clk; child u(.clk(clk)); endmodule\n";

        let hints = port_hints(text);
        assert_eq!(hints.iter().map(|hint| hint.label.as_str()).collect::<Vec<_>>(), vec!["→"]);
        assert!(hints[0].target_location.is_none());
    }

    #[test]
    fn named_port_hint_is_clickable() {
        let text = "module child(output out); endmodule\n\
            module top; logic instr_addr_o; child u(instr_addr_o); endmodule\n";

        let hints = port_hints(text);
        assert_eq!(hints.iter().map(|hint| hint.label.as_str()).collect::<Vec<_>>(), vec!["out →"]);
        assert!(hints[0].target_location.is_some());
    }

    #[test]
    fn ordered_port_hint_keeps_different_name_with_direction() {
        let text = "module child(output out); endmodule\n\
            module top; logic instr_addr_o; child u(instr_addr_o); endmodule\n";

        assert_eq!(port_hint_labels(text), vec!["out →"]);
    }

    #[test]
    fn named_port_hint_omits_visible_name_but_keeps_direction() {
        let text = "module child(output instr_addr_o); endmodule\n\
            module top; logic instr_addr_o; child u(.instr_addr_o(instr_addr_o)); endmodule\n";

        assert_eq!(port_hint_labels(text), vec!["→"]);
    }

    #[test]
    fn named_port_hints_resolve_ports_by_name_not_position() {
        let text = "module child(input a, output b, input c); endmodule\n\
            module top; logic local_b, local_c; child u(.b(local_b), .c(local_c)); endmodule\n";

        assert_eq!(port_hint_labels(text), vec!["→", "←"]);
    }

    #[test]
    fn port_hints_in_later_viewport_skip_previous_connections() {
        let text = "module child(input a, output b); endmodule\n\
            module top; logic local_a, local_b; child u(.a(local_a), .b(local_b)); endmodule\n";
        let start = TextSize::from(text.find(".b(local_b)").expect("second connection") as u32);
        let end = start + TextSize::of(".b(local_b)");

        assert_eq!(port_hint_labels_in_range(text, TextRange::new(start, end)), vec!["→"]);
    }

    #[test]
    fn unknown_named_port_does_not_fall_back_to_position() {
        let text = "module child(input a); endmodule\n\
            module top; logic sig; child u(.bogus(sig)); endmodule\n";

        assert_eq!(port_hint_labels(text), Vec::<String>::new());
    }

    #[test]
    fn unknown_named_non_ansi_port_does_not_fall_back_to_position() {
        let text = "module child(a); input a; endmodule\n\
            module top; logic sig; child u(.bogus(sig)); endmodule\n";

        assert_eq!(port_hint_labels(text), Vec::<String>::new());
    }

    #[test]
    fn explicit_non_ansi_port_label_uses_internal_ref_for_direction() {
        let text = "module child(.out(foo)); output foo; endmodule\n\
            module top; logic sig; child u(.out(sig)); endmodule\n";

        assert_eq!(port_hint_labels(text), vec!["→"]);
    }

    #[test]
    fn implicit_named_port_hint_appears_before_local_name() {
        let text = "module child(input clk_i); endmodule\n\
            module top; logic clk_i; child u(.clk_i,); endmodule\n";

        let hints = port_hints(text);
        assert_eq!(hints.iter().map(|hint| hint.label.as_str()).collect::<Vec<_>>(), vec!["←"]);
        assert_eq!(
            hints[0].position,
            TextSize::from(text.rfind("clk_i,").expect("connection name") as u32)
        );
        assert!(hints[0].target_location.is_none());
    }

    #[test]
    fn extra_ordered_parameter_assignments_do_not_invent_param_hints() {
        let text = "module child #(parameter P = 1) (); endmodule\nmodule top; child #(1, 2) u(); endmodule\n";

        assert_eq!(param_hint_labels(text), vec!["P:"]);
    }
}
