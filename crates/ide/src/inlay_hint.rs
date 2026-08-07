use hir_def::{
    ast_id_map::SourceAstId,
    body::{Body, BodyItem},
    container::{InContainer, InFile},
    def_id::DefId,
    expr::Expr,
    module::{
        ModuleId,
        instantiation::{Instantiation, ParamAssign, PortConn, PortConnId},
        port::PortDirection,
    },
    source_map::{Lowered, SourceInfo},
};
use preproc_expand::{
    file::HirFileId,
    preproc::{MacroCallResolution, macro_call_resolutions_in_range},
};
use syntax::{
    SyntaxTokenWithParent,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
    match_ast_kind,
};
use utils::{
    check_or_throw,
    text_edit::{TextEdit, TextRange, TextSize},
};
use vfs::FileId;

use crate::{
    db::root_db::RootDb,
    markup::Markup,
    module_resolution::{resolve_connection_port, resolve_module_name, resolve_port_metadata},
    references::search::resolve_source_range,
};

#[derive(Debug)]
pub struct InlayHintConfig {
    pub port_connection: bool,
    pub parameter_assignment: bool,
    pub macro_argument: bool,
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
    MacroArgument,
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
    fn from_src(src: SourceInfo, position: Option<TextSize>) -> Option<Self> {
        let range = src.full_range();
        let kind = match_ast_kind! { src.kind()?,
            ast::ParamAssignment => InlayKind::ParamAssign,
            ast::OrderedPortConnection | ast::EmptyPortConnection | ast::NamedPortConnection => InlayKind::Port,
            _ => return None,
        };
        let (padding_left, padding_right) = match_ast_kind! { src.kind()?,
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

    fn macro_argument(range: TextRange) -> Self {
        Self {
            range,
            position: range.start(),
            kind: InlayKind::MacroArgument,
            padding_left: false,
            padding_right: true,
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
        target_location: Option<InFile<TextRange>>,
        label: String,
        text_edit: Option<TextEdit>,
    ) {
        if !self.intersect(anchor.range) {
            return;
        }

        let tooltip = target_location.as_ref().map(|_| Markup::new());

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
        src: SourceInfo,
        target_location: Option<InFile<TextRange>>,
        position: Option<TextSize>,
        label: String,
        text_edit: Option<TextEdit>,
    ) {
        if let Some(anchor) = HintAnchor::from_src(src, position) {
            self.collect_hint(anchor, target_location, label, text_edit);
        }
    }

    fn collect_range_hint(
        &mut self,
        anchor: HintAnchor,
        target_location: Option<InFile<TextRange>>,
        label: String,
    ) {
        if !self.intersect(anchor.range) {
            return;
        }

        let tooltip = target_location.as_ref().map(|_| Markup::new());
        self.hints.push(InlayHint {
            label,
            tooltip,
            target_location,
            padding_left: anchor.padding_left,
            padding_right: anchor.padding_right,
            position: anchor.position,
            kind: anchor.kind,
            text_edit: None,
        });
    }

    fn collect_module_end_hint(&mut self, end_range: TextRange, name: &str) {
        self.collect_hint(HintAnchor::module_end(end_range), None, format!(": {name}"), None);
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
    let _span = tracing::debug_span!("ide.inlay_hint", ?file_id, ?range).entered();
    let file_id = HirFileId::File(file_id);
    let file = db.hir_file_with_source_map(file_id);

    let mut collector = InlayHintCollector::new(range, config);

    if collector.config.macro_argument {
        collect_macro_argument_hints(db, file_id.expect_file(), range, &mut collector);
    }

    for item in &file.data_ref().items {
        #[allow(clippy::single_match)]
        match item.clone() {
            BodyItem::LocalModuleId(idx) => {
                let module_id = ModuleId::new(file_id, idx);
                let Some(module_src) = file.source(idx) else {
                    continue;
                };

                if file.source_range(db, idx).is_some_and(|range| collector.intersect(range)) {
                    collect_module_items(db, module_id, module_src, &mut collector);
                }
            }
            _ => {}
        }
    }

    let mut hints = collector.into_hints();
    for hint in &mut hints {
        if let Some(loc) = hint.target_location {
            hint.target_location = resolve_source_range(db, loc.file_id, loc.value)
                .map(|(file_id, range)| InFile::new(HirFileId::File(file_id), range));
        }
    }
    hints
}

fn collect_macro_argument_hints(
    db: &RootDb,
    file_id: FileId,
    range: TextRange,
    collector: &mut InlayHintCollector,
) {
    let Ok(resolutions) = macro_call_resolutions_in_range(db, file_id, range) else {
        return;
    };

    for resolution in resolutions {
        collect_macro_argument_hints_for_call(resolution, collector);
    }
}

fn collect_macro_argument_hints_for_call(
    resolution: MacroCallResolution,
    collector: &mut InlayHintCollector,
) -> Option<()> {
    let params = resolution.definition.params.as_ref()?;
    for argument in &resolution.call.arguments {
        let Some(argument_range) = argument.range else {
            continue;
        };
        let Some(param) = params.get(argument.argument_index) else {
            continue;
        };
        let Some(param_name) = &param.name else {
            continue;
        };
        let Some(param_range) = param.range else {
            continue;
        };
        collector.collect_range_hint(
            HintAnchor::macro_argument(argument_range),
            Some(InFile::new(HirFileId::File(resolution.definition.file_id), param_range)),
            format!("{param_name}:"),
        );
    }

    Some(())
}

fn collect_module_items(
    db: &RootDb,
    module_id: ModuleId,
    module_src: SourceAstId,
    collector: &mut InlayHintCollector,
) {
    let module = db.module_with_source_map(module_id);

    if collector.config.instantiation() {
        for (instantiation_id, instantiation) in module.instantiations.iter() {
            let Some(instantiation_src) = module.source_info(db, instantiation_id) else {
                continue;
            };
            if collector.intersect(instantiation_src.full_range()) {
                process_instantiation(db, module_id, &module, instantiation, collector);
            }
        }
    }

    if collector.config.end_structure
        && let Some(name) = &module.name
    {
        if let Some(end_range) = module_end_range(db, module_id.file_id, module_src) {
            collector.collect_module_end_hint(end_range, name);
        }
    }
}

fn module_end_range(db: &RootDb, file_id: HirFileId, source: SourceAstId) -> Option<TextRange> {
    let tree = db.parse(file_id);
    let module = ast::ModuleDeclaration::cast(db.ast_id_map(file_id).node(source, &tree)?)?;
    SyntaxTokenWithParent { parent: module.syntax(), tok: module.endmodule()? }.text_range()
}

fn process_instantiation(
    db: &RootDb,
    module_id: ModuleId,
    module: &Lowered<Body>,
    instantiation: &Instantiation,
    collector: &mut InlayHintCollector,
) -> Option<()> {
    let module_body = db.module_body_with_source_map(module_id);
    let from_file = module_id.file_id.source_file_id(db)?;
    let target_module_id =
        resolve_module_name(db, from_file, instantiation.module_name.as_ref()?).unique()?;

    let target_module = db.module_with_source_map(target_module_id);
    let target_body = db.module_body_with_source_map(target_module_id);

    // handle param assignments
    if collector.config.parameter_assignment {
        for (id, &assign_id) in instantiation.param_assigns.iter().enumerate() {
            try {
                let ParamAssign::Ordered(assign_expr) = module.get(assign_id) else {
                    continue;
                };
                let assign_src = module.source_info(db, assign_id)?;
                check_or_throw!(collector.intersect(assign_src.full_range()));

                let param_id = hir_def::module::overridable_param_id_by_idx(&target_body, id)?;
                let param_def = DefId::new(
                    db,
                    InContainer::new(
                        target_module_id.owner(db).expect("target module owner"),
                        param_id,
                    ),
                );
                let param_name = param_def.primary_origin(db).name(db)?;
                check_or_throw!(!should_skip(module_body.get(*assign_expr), &param_name));
                let target_range = param_def.primary_origin(db).range(db)?;
                collector.collect_src_hint(
                    assign_src,
                    Some(target_range),
                    None,
                    format!("{param_name}:"),
                    edits_for_conn(&param_name, assign_src),
                );
            };
        }
    }

    // handle port connections
    if collector.config.port_connection {
        for instance_id in instantiation.instances.iter() {
            let instance = module.get(*instance_id);
            let Some(instance_range) = module.source_range(db, *instance_id) else {
                continue;
            };
            if !collector.intersect(instance_range) {
                continue;
            }

            for (idx, &conn_id) in instance.connections.iter().enumerate() {
                try {
                    let conn = module.get(conn_id);
                    check_or_throw!(
                        module
                            .source_range(db, conn_id)
                            .is_some_and(|range| collector.intersect(range))
                    );

                    let def = resolve_connection_port(db, target_module_id, conn, idx).unique()?;
                    let (name, dir, _ty) =
                        resolve_port_metadata(db, &target_module, &target_body, &def.origins(db))?;
                    let dir = dir?;
                    let target_range = def.primary_origin(db).range(db)?;
                    collect_connection_hint(
                        db,
                        module,
                        &module_body,
                        conn_id,
                        name,
                        dir,
                        target_range,
                        collector,
                    );
                };
            }
        }
    }

    Some(())
}

fn collect_connection_hint(
    db: &RootDb,
    module: &Lowered<Body>,
    body: &Lowered<Body>,
    conn_id: PortConnId,
    name: &str,
    port_dir: PortDirection,
    target_range: InFile<TextRange>,
    collector: &mut InlayHintCollector,
) -> Option<()> {
    let conn = module.get(conn_id);
    let conn_src = module.named_source_info(db, conn_id)?;
    let arrow = match port_dir {
        PortDirection::Input => "←",
        PortDirection::Output => "→",
        PortDirection::Inout => "↔",
        PortDirection::Ref => "&",
    };

    let conn_start = conn_src.full_range().start();
    match conn {
        PortConn::Empty => {
            let label = format!("{name} {arrow}");
            let edit = edits_for_conn(name, conn_src);
            collector.collect_src_hint(conn_src, Some(target_range), None, label, edit);
        }
        PortConn::Ordered(expr) => {
            let same_name = should_skip(body.get(*expr), name);
            let label = if same_name { arrow.to_string() } else { format!("{name} {arrow}") };
            let target_range = if same_name { None } else { Some(target_range) };
            let edit = if same_name { None } else { edits_for_conn(name, conn_src) };
            let position = body.source_range(db, *expr).map_or(conn_start, |range| range.start());
            collector.collect_src_hint(conn_src, target_range, Some(position), label, edit);
        }
        PortConn::Named(port_name, expr) => {
            let (label, target_range) =
                if port_name.as_ref().is_none_or(|port_name| port_name != name) {
                    (format!("{name} {arrow}"), Some(target_range))
                } else {
                    (arrow.to_string(), None)
                };
            let position = expr
                .and_then(|expr| body.source_range(db, expr).map(|range| range.start()))
                .or_else(|| conn_src.focus_range().map(|range| range.start()))
                .unwrap_or(conn_start);
            collector.collect_src_hint(conn_src, target_range, Some(position), label, None);
        }
        PortConn::Wildcard => {}
    }

    Some(())
}

fn edits_for_conn(param: &str, conn_src: SourceInfo) -> Option<TextEdit> {
    let mut builder = TextEdit::builder();
    builder.insert(conn_src.full_range().start(), format!(".{}(", param));
    builder.insert(conn_src.full_range().end(), String::from(")"));
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
    use utils::text_edit::{TextRange, TextSize};
    use vfs::{ChangedFile, FileId, FileSet, VfsPath};

    use super::{InlayHintConfig, inlay_hint};
    use crate::db::root_db::RootDb;

    fn db_with_file(text: &str) -> (RootDb, FileId) {
        let file_id = FileId::from_raw(0);
        let path = VfsPath::new_virtual_path("/test.sv".to_owned());

        let mut file_set = FileSet::default();
        file_set.insert(file_id, path);
        let root = SourceRoot::new_local(file_set);

        let mut change = Change::new();
        change.set_roots(vec![root]);
        change.add_changed_file(ChangedFile::create(file_id, text));

        let mut db = RootDb::new(None);
        change.apply(&mut db);
        (db, file_id)
    }

    fn port_config() -> InlayHintConfig {
        InlayHintConfig {
            port_connection: true,
            parameter_assignment: false,
            macro_argument: false,
            end_structure: false,
        }
    }

    fn parameter_config() -> InlayHintConfig {
        InlayHintConfig {
            port_connection: false,
            parameter_assignment: true,
            macro_argument: false,
            end_structure: false,
        }
    }

    fn macro_argument_config() -> InlayHintConfig {
        InlayHintConfig {
            port_connection: false,
            parameter_assignment: false,
            macro_argument: true,
            end_structure: false,
        }
    }

    fn end_structure_config() -> InlayHintConfig {
        InlayHintConfig {
            port_connection: false,
            parameter_assignment: false,
            macro_argument: false,
            end_structure: true,
        }
    }

    struct InlayFixture {
        source: String,
        range: Option<TextRange>,
        config: InlayHintConfig,
    }

    fn read_fixture(path: &std::path::Path) -> InlayFixture {
        let raw = std::fs::read_to_string(path)
            .unwrap_or_else(|err| panic!("failed to read fixture {}: {err}", path.display()));
        let mut offset = 0;
        let mut config = None;

        while offset < raw.len() {
            let rest = &raw[offset..];
            let line_len = rest.find('\n').map_or(rest.len(), |idx| idx + 1);
            let line_with_newline = &rest[..line_len];
            let line = line_with_newline.strip_suffix('\n').unwrap_or(line_with_newline);
            let Some(meta) = line.trim().strip_prefix("//- ") else {
                break;
            };
            let (key, value) = meta
                .split_once(':')
                .unwrap_or_else(|| panic!("invalid metadata in {}", path.display()));
            match key.trim() {
                "config" => config = Some(parse_config(value.trim(), path)),
                other => panic!("unknown metadata key `{other}` in {}", path.display()),
            }
            offset += line_len;
        }

        let (source, range) = strip_range_markers(&raw[offset..], path);
        let range = range.or_else(|| Some(TextRange::up_to(TextSize::of(source.as_str()))));
        InlayFixture {
            source,
            range,
            config: config.unwrap_or_else(|| panic!("missing config in {}", path.display())),
        }
    }

    fn parse_config(value: &str, path: &std::path::Path) -> InlayHintConfig {
        match value {
            "port" => port_config(),
            "parameter" => parameter_config(),
            "macro_argument" => macro_argument_config(),
            "end_structure" => end_structure_config(),
            other => panic!("unknown config `{other}` in {}", path.display()),
        }
    }

    fn strip_range_markers(text: &str, path: &std::path::Path) -> (String, Option<TextRange>) {
        const START: &str = "/*range-start*/";
        const END: &str = "/*range-end*/";

        let Some(start_marker) = text.find(START) else {
            if text.contains(END) {
                panic!("range end without start in {}", path.display());
            }
            return (text.to_string(), None);
        };
        let after_start = start_marker + START.len();
        let end_marker = text[after_start..]
            .find(END)
            .map(|idx| after_start + idx)
            .unwrap_or_else(|| panic!("range start without end in {}", path.display()));

        let start = TextSize::of(&text[..start_marker]);
        let end = start + TextSize::of(&text[after_start..end_marker]);
        let mut source = String::new();
        source.push_str(&text[..start_marker]);
        source.push_str(&text[after_start..end_marker]);
        source.push_str(&text[end_marker + END.len()..]);
        (source, Some(TextRange::new(start, end)))
    }

    fn hint_snapshot(hints: Vec<super::InlayHint>) -> String {
        if hints.is_empty() {
            return String::from("<none>");
        }

        let mut out = String::new();
        for hint in hints {
            let target = hint
                .target_location
                .as_ref()
                .map(|target| (usize::from(target.value.start()), usize::from(target.value.end())));
            let edit = hint.text_edit.as_ref().map(|edit| format!("{edit:?}"));
            out.push_str(&format!(
                "{:?} @ {} {:?} padding=({}, {}) target={:?} edit={:?}\n",
                hint.kind,
                usize::from(hint.position),
                hint.label,
                hint.padding_left,
                hint.padding_right,
                target,
                edit
            ));
        }
        out
    }

    #[test]
    fn inlay_hint_fixtures() {
        insta::glob!("inlay_hint/fixtures/*.sv", |path| {
            let fixture = read_fixture(path);
            let (db, file_id) = db_with_file(&fixture.source);
            let hints = inlay_hint(
                &db,
                file_id,
                fixture.range.expect("fixture range should be initialized"),
                fixture.config,
            );
            insta::assert_snapshot!(hint_snapshot(hints));
        });
    }
}
