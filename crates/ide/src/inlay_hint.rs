use hir::{
    container::InFile,
    db::HirDb,
    file::HirFileId,
    hir_def::{
        expr::Expr,
        file::FileItem,
        module::{
            Module, ModuleId, ModuleSourceMap, ModuleSrc,
            instantiation::{Instantiation, ParamAssign, PortConn},
            port::Ports,
        },
    },
    scope::UnitEntry,
    source_map::IsSrc,
};
use ide_db::root_db::RootDb;
use syntax::{ast, match_ast_kind};
use utils::{
    get::{Get, GetRef},
    text_edit::{TextEdit, TextRange, TextSize},
};
use vfs::FileId;

use crate::markup::Markup;

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
        src: impl IsSrc,
        target_src: Option<InFile<impl IsSrc>>,
        label: String,
        text_edit: Option<TextEdit>,
    ) {
        let range = src.range();
        assert!(range.intersect(self.range).is_some());

        let kind = match_ast_kind! { src.kind(),
            ast::ParamAssignment => InlayKind::ParamAssign,
            ast::OrderedPortConnection | ast::EmptyPortConnection => InlayKind::Port,
            ast::ModuleDeclaration => InlayKind::EndStructure,
            _ => return,
        };

        let position = match_ast_kind! { src.kind(),
            ast::ModuleDeclaration => range.end(),
            _ => range.start(),
        };

        let (padding_left, padding_right) = match_ast_kind! { src.kind(),
            ast::ModuleDeclaration => (true, false),
            _ => (false, false),
        };

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
            padding_left,
            padding_right,
            position,
            kind,
            text_edit,
        });
    }

    fn collect_port_hint(
        &mut self,
        name: &str,
        conn_src: impl IsSrc,
        target_src: InFile<impl IsSrc>,
    ) {
        self.collect_hint(
            conn_src,
            Some(target_src),
            format!("{name}: "),
            edits_for_conn(name, conn_src),
        );
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
                process_instantiation(db, module, src_map, instantiation, collector);
            }
        }
    }

    if collector.config.end_structure
        && let Some(name) = &module.name
    {
        collector.collect_hint(module_src, None::<InFile<ModuleSrc>>, format!(": {name}"), None);
    }
}

fn process_instantiation(
    db: &RootDb,
    module: &Module,
    src_map: &ModuleSourceMap,
    instantiation: &Instantiation,
    collector: &mut InlayHintCollector,
) -> Option<()> {
    let unit_scope = db.unit_scope();
    let target_module_id = match unit_scope.get(instantiation.module_name.as_ref()?) {
        Some(UnitEntry::ModuleId(module_id)) => module_id,
        _ => return None,
    };

    let target_file = target_module_id.file_id;
    let (target_module, target_src_map) = db.module_with_source_map(target_module_id);

    // handle param assignments
    if collector.config.parameter_assignment {
        for (id, &assign_id) in instantiation.param_assigns.iter().enumerate() {
            let ParamAssign::Ordered(assign_expr) = module.get(assign_id) else {
                continue;
            };
            let Some(assign_src) = src_map.get(assign_id) else {
                continue;
            };
            if !collector.intersect(assign_src.range()) {
                break;
            }

            let Some(param_id) = target_module.param_port_id_by_idx(id) else {
                continue;
            };
            let Some(param_name) = target_module.get(param_id).name.as_ref() else {
                continue;
            };

            if should_skip(module.get(*assign_expr), param_name) {
                continue;
            }

            let Some(target_src) = target_src_map.get(param_id) else {
                continue;
            };
            let target_src = InFile::new(target_file, target_src);
            collector.collect_port_hint(param_name, assign_src, target_src);
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
                break;
            }

            for (id, &conn_id) in instance.connections.iter().enumerate() {
                let conn_expr = match module.get(conn_id) {
                    PortConn::Empty => None,
                    PortConn::Ordered(expr) => Some(expr),
                    PortConn::Named(..) | PortConn::Wildcard => continue,
                };

                let Some(conn_src) = src_map.get(conn_id) else {
                    continue;
                };
                if !collector.intersect(conn_src.range()) {
                    break;
                }

                match &target_module.ports {
                    Ports::NonAnsi { .. } => {
                        let Some(port_id) = target_module.non_ansi_port_id_by_idx(id) else {
                            continue;
                        };
                        let Some(port_name) = target_module.get(port_id).label.as_ref() else {
                            continue;
                        };

                        if conn_expr.is_some_and(|expr| should_skip(module.get(*expr), port_name)) {
                            continue;
                        }

                        let Some(target_src) = target_src_map.get(port_id) else {
                            continue;
                        };
                        let target_src = InFile::new(target_file, target_src);
                        collector.collect_port_hint(port_name, conn_src, target_src);
                    }
                    Ports::Ansi(_) => {
                        let Some(port_decl_id) = target_module.ansi_port_decl_id_by_idx(id) else {
                            continue;
                        };
                        let port_decl = target_module.get(port_decl_id);
                        let Some(port_name) = port_decl.name.as_ref().or_else(|| {
                            port_decl
                                .decls
                                .clone()
                                .next()
                                .and_then(|decl_id| target_module.get(decl_id).name.as_ref())
                        }) else {
                            continue;
                        };

                        if conn_expr.is_some_and(|expr| should_skip(module.get(*expr), port_name)) {
                            continue;
                        }

                        let Some(target_src) = target_src_map.get(port_decl_id) else {
                            continue;
                        };
                        let target_src = InFile::new(target_file, target_src);
                        collector.collect_port_hint(port_name, conn_src, target_src);
                    }
                }
            }
        }
    }

    Some(())
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
    use base_db::{change::Change, source_root::SourceRoot};
    use ide_db::root_db::RootDb;
    use triomphe::Arc;
    use utils::{
        lines::LineEnding,
        text_edit::{TextRange, TextSize},
    };
    use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

    use super::{InlayHintConfig, InlayKind, inlay_hint};

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

    fn port_hint_labels(text: &str) -> Vec<String> {
        let (db, file_id) = db_with_file(text);
        let range = TextRange::new(TextSize::from(0), TextSize::of(text));
        inlay_hint(&db, file_id, range, port_config())
            .into_iter()
            .filter(|hint| matches!(hint.kind, InlayKind::Port))
            .map(|hint| hint.label)
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
    fn extra_ordered_connections_do_not_invent_ansi_port_hints() {
        let text = "module child(input a); endmodule\nmodule top; child u(1'b0, 1'b1); endmodule\n";

        assert_eq!(port_hint_labels(text), vec!["a: "]);
    }

    #[test]
    fn extra_ordered_connections_do_not_invent_non_ansi_port_hints() {
        let text =
            "module child(a); input a; endmodule\nmodule top; child u(1'b0, 1'b1); endmodule\n";

        assert_eq!(port_hint_labels(text), vec!["a: "]);
    }

    #[test]
    fn extra_ordered_parameter_assignments_do_not_invent_param_hints() {
        let text = "module child #(parameter P = 1) (); endmodule\nmodule top; child #(1, 2) u(); endmodule\n";

        assert_eq!(param_hint_labels(text), vec!["P: "]);
    }
}
