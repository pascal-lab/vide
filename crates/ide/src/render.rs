use hir::{
    base_db::{
        intern::Lookup,
        source_db::{SourceDb, SourceRootDb},
    },
    container::{ContainerId, ContainerParent, InContainer, InFile, InModule, InSubroutine},
    db::HirDb,
    display::HirDisplay,
    hir_def::{
        DEFAULT_NAME,
        declaration::Declaration,
        expr::{
            data_ty::DataTy,
            declarator::{DeclId, DeclaratorParent},
        },
        literal::Literal,
        module::{
            ModuleId,
            instantiation::InstanceId,
            port::{NonAnsiPortId, Ports},
        },
        subroutine::{SubroutineId, SubroutineKind, SubroutinePortId},
    },
    region_tree::RegionParent,
    semantics::Semantics,
    source_map::IsSrc,
};
use itertools::Itertools;
use syntax::{
    SVInt, SyntaxCursorExt, SyntaxKind, SyntaxNodeExt,
    has_text_range::HasTextRange,
    token::SyntaxTokenWithParentExt,
    trivia::{TriviaExt, TriviaKindExt},
};
use utils::get::{Get, GetRef};

use crate::{
    db::{line_index_db::LineIndexDb, root_db::RootDb},
    definitions::{Definition, DefinitionOrigin},
    markup::Markup,
    module_resolution::resolve_module_name,
};

pub(crate) fn render_literal(literal: &Literal) -> Option<Markup> {
    let mut res = Markup::new();

    match literal {
        Literal::Int(svint) => {
            let width = svint.get_bit_width();
            let dec = render_svint(svint, 10);
            let mut info = format!("{dec} ({width} bits)");
            if let Some(ieee754) = render_svint_as_ieee754(svint) {
                info.push_str(&format!("\nieee754: {ieee754}"));
            }

            res.push_with_plain_fence(&info);
            res.new_section("Radix");

            let bin = render_svint(svint, 2);
            let oct = render_svint(svint, 8);
            let hex = render_svint(svint, 16);
            res.push_with_plain_fence(&format!("bin: {bin}\nhex: {hex}\noct: {oct}",));
        }
        Literal::Float(float) => {
            let num = f64::from(*float);
            let bits = float.to_bits();
            res.push_with_plain_fence(&format!("{num}\nbits: {bits:#x}"));
        }
        Literal::Time { val, unit } => {
            let num = f64::from(*val);
            res.push_with_plain_fence(&format!("{num} {unit}"));
        }
        Literal::Str(s) => {
            res.push_with_plain_fence(&format!("{s}"));
        }
        Literal::UnbasedUnsized(bit) => {
            res.push_with_plain_fence(&format!("{bit}"));
        }
    };

    Some(res)
}

fn render_svint(svint: &SVInt, base: usize) -> String {
    let mut s = svint.serialize(base);
    let mut len = s.len();
    let width = svint.get_bit_width();
    if base == 2 || base == 8 || base == 16 {
        let log = match base {
            2 => 1,
            8 => 3,
            16 => 4,
            _ => return s,
        };
        s.insert_str(0, &"0".repeat(width.div_ceil(log) - len));
        len += width.div_ceil(log) - len;
    }

    let interval = match base {
        2 => 4,
        8 => 3,
        10 => 3,
        16 => 4,
        _ => return s,
    };

    let mut result = String::with_capacity(len + len / interval + len / 4);

    for (i, c) in s.chars().enumerate() {
        if i > 0 {
            if base == 2 && (len - i).is_multiple_of(16) {
                result.push_str(" / ");
            } else if (len - i).is_multiple_of(interval) {
                result.push(' ');
            }
        }
        result.push(c);
    }

    result
}

fn render_svint_as_ieee754(svint: &SVInt) -> Option<String> {
    let width = svint.get_bit_width();

    if (width != 32 && width != 64) || svint.has_unknown() {
        return None;
    }

    let word = svint.get_single_word()?;
    if width == 32 {
        let f = f32::from_bits(word as u32);
        Some(format!("{:?}", f))
    } else {
        let f = f64::from_bits(word);
        Some(format!("{:?}", f))
    }
}

pub(crate) fn render_definition(sema: &Semantics<RootDb>, def: Definition) -> Markup {
    def.def_origins().into_iter().fold(Markup::new(), |mut res, origin| {
        let origin = render_def_origin(sema, &origin);

        if !res.is_empty() && !origin.is_empty() {
            res.newline();
        }

        res.merge(origin);
        res
    })
}

pub(crate) fn render_definition_location(sema: &Semantics<RootDb>, def: Definition) -> Markup {
    let db = sema.db;
    let mut locations = def
        .def_origins()
        .into_iter()
        .filter_map(|origin| render_def_origin_location(db, &origin))
        .collect_vec();
    locations.sort();
    locations.dedup();

    let mut res = Markup::new();
    for (idx, location) in locations.into_iter().enumerate() {
        if idx > 0 {
            res.print("\n");
        }
        res.print(&location);
    }
    res
}

fn render_def_origin_location(db: &RootDb, origin: &DefinitionOrigin) -> Option<String> {
    let InFile { value: range, file_id } = origin.range(db)?;
    let file_id = file_id.file_id();
    let source_root = db.source_root(db.source_root_id(file_id));
    let path = source_root
        .path_for_file(&file_id)
        .map(ToString::to_string)
        .or_else(|| db.file_path(file_id).map(|path| path.to_string()))
        .unwrap_or_else(|| format!("{file_id:?}"));
    let line = db.line_index(file_id).try_line_col(range.start())?.line + 1;

    Some(format!("{path}:{line}"))
}

fn render_def_origin(sema: &Semantics<RootDb>, origin: &DefinitionOrigin) -> Markup {
    let mut res = Markup::new();
    let mut has_signature = false;

    if let Some(signature) = render_signature(sema, origin) {
        res.push_with_code_fence(&signature);
        has_signature = true;
    }

    let containers = render_containers(sema, origin);
    if has_signature && !containers.is_empty() {
        res.horizontal_line();
    }
    res.merge(containers);

    if let Some(markup) = render_side_comments(sema, origin) {
        if !res.is_empty() {
            res.horizontal_line();
        }
        res.merge(markup);
    }

    res
}

fn render_signature(sema: &Semantics<RootDb>, origin: &DefinitionOrigin) -> Option<String> {
    let db = sema.db;
    match origin {
        DefinitionOrigin::ModuleId(module_id) => render_module_signature(db, *module_id),
        DefinitionOrigin::SubroutineId(subroutine_id) => {
            render_subroutine_signature(db, *subroutine_id)
        }
        DefinitionOrigin::SubroutinePort(port_id) => render_subroutine_port_signature(db, *port_id),
        DefinitionOrigin::NonAnsiPort(port_id) => render_non_ansi_port_signature(db, *port_id),
        DefinitionOrigin::Decl(decl_id) => render_decl_signature(db, *decl_id),
        DefinitionOrigin::Typedef(typedef) => typedef.display_signature(db).ok(),
        DefinitionOrigin::Instance(instance_id) => render_instance_signature(db, *instance_id),
        _ => render_label_signature(db, origin),
    }
}

fn render_module_signature(db: &RootDb, module_id: ModuleId) -> Option<String> {
    let module = db.module(module_id);
    let name = module.name.as_ref()?;
    let src = module_id.file_id.to_container_src_map(db).get(module_id.value)?;
    let kind = if src.kind() == SyntaxKind::INTERFACE_DECLARATION { "interface" } else { "module" };
    let mut signature = format!("{kind} {name}");

    let params = render_module_param_ports(db, module_id);
    if !params.is_empty() {
        signature.push_str(" #(\n");
        signature.push_str(&render_indented_list(&params));
        signature.push_str("\n)");
    }

    let ports = render_module_port_list(db, module_id);
    if ports.is_empty() {
        signature.push_str(" ()");
    } else {
        signature.push_str(" (\n");
        signature.push_str(&render_indented_list(&ports));
        signature.push_str("\n)");
    }
    Some(signature)
}

fn render_module_param_ports(db: &RootDb, module_id: ModuleId) -> Vec<String> {
    let module = db.module(module_id);
    let mut params = Vec::new();
    let mut idx = 0;
    while let Some(decl_id) = module.param_port_id_by_idx(idx) {
        let decl = module.get(decl_id);
        let DeclaratorParent::DeclarationId(parent) = decl.parent else {
            idx += 1;
            continue;
        };
        let Some(prefix) = render_declaration_prefix(db, module_id.into(), module.get(parent))
        else {
            idx += 1;
            continue;
        };
        let Some(decl) = InContainer::new(module_id.into(), decl_id).display_signature(db).ok()
        else {
            idx += 1;
            continue;
        };
        let init =
            render_initializer(db, InContainer::new(module_id.into(), decl_id)).unwrap_or_default();
        params.push(format!("{prefix} {decl}{init}"));
        idx += 1;
    }
    params
}

fn render_subroutine_port_signature(
    db: &RootDb,
    port_id: InSubroutine<SubroutinePortId>,
) -> Option<String> {
    let subroutine = db.subroutine(port_id.subroutine);
    let port = subroutine.ports.get(port_id.value.0 as usize)?;
    let name = port.name.as_ref()?;
    let container = port_id.subroutine.lookup(db).cont_id.into();
    let ty = port.ty.and_then(|ty| render_data_ty(db, container, ty));
    let dir = port.direction.display_source(db).ok()?;

    match (dir.is_empty(), ty) {
        (false, Some(ty)) => Some(format!("{dir} {ty} {name}")),
        (false, None) => Some(format!("{dir} {name}")),
        (true, Some(ty)) => Some(format!("{ty} {name}")),
        (true, None) => Some(name.to_string()),
    }
}

fn render_subroutine_signature(db: &RootDb, subroutine_id: SubroutineId) -> Option<String> {
    let subroutine = db.subroutine(subroutine_id);
    let name = subroutine.name.as_ref()?;
    let container = subroutine_id.lookup(db).cont_id.into();
    let mut signature = match subroutine.kind {
        SubroutineKind::Task => format!("task {name}"),
        SubroutineKind::Function { return_ty } => {
            if let Some(return_ty) = return_ty.and_then(|ty| render_data_ty(db, container, ty)) {
                format!("function {return_ty} {name}")
            } else {
                format!("function {name}")
            }
        }
    };

    let ports = subroutine
        .ports
        .iter()
        .enumerate()
        .filter_map(|(idx, _)| {
            render_subroutine_port_signature(
                db,
                InSubroutine::new(subroutine_id, SubroutinePortId(idx as u32)),
            )
        })
        .collect_vec();
    if ports.is_empty() {
        signature.push_str("()");
    } else {
        signature.push_str("(\n");
        signature.push_str(&render_indented_list(&ports));
        signature.push_str("\n)");
    }
    Some(signature)
}

fn render_module_port_list(db: &RootDb, module_id: ModuleId) -> Vec<String> {
    let module = db.module(module_id);
    match &module.ports {
        Ports::NonAnsi { ports, .. } => ports
            .values()
            .map(|port| {
                port.label
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "<missing>".to_string())
            })
            .collect_vec(),
        Ports::Ansi(port_decls) => {
            let mut ports = Vec::new();
            for port_decl in port_decls.values() {
                let header =
                    InModule::new(module_id, port_decl.header.clone()).display_source(db).ok();
                for decl_id in port_decl.decls.clone() {
                    let name =
                        InContainer::new(module_id.into(), decl_id).display_signature(db).ok();
                    match (header.as_deref(), name.as_deref()) {
                        (Some(header), Some(name)) if !header.is_empty() => {
                            ports.push(format!("{header} {name}"));
                        }
                        (_, Some(name)) => ports.push(name.to_string()),
                        _ => {}
                    }
                }
            }
            ports
        }
    }
}

fn render_indented_list(items: &[String]) -> String {
    items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let suffix = if idx + 1 == items.len() { "" } else { "," };
            format!("    {item}{suffix}")
        })
        .collect_vec()
        .join("\n")
}

fn render_non_ansi_port_signature(db: &RootDb, port_id: InModule<NonAnsiPortId>) -> Option<String> {
    let module = db.module(port_id.module_id);
    let port = module.get(port_id.value);
    let label = port.label.as_ref()?;
    Some(format!("port {label}"))
}

fn render_instance_signature(db: &RootDb, instance_id: InModule<InstanceId>) -> Option<String> {
    let parent_module = db.module(instance_id.module_id);
    let instance = parent_module.get(instance_id.value);
    let instance_name = instance.name.as_ref()?;
    let instantiation = parent_module.get(instance.parent);
    let module_name = instantiation.module_name.as_ref()?;

    let mut signature = format!("instance {instance_name} of {module_name}");
    if let Some(target_module_id) =
        resolve_module_name(db, instance_id.module_id.file_id.file_id(), module_name).unique()
        && let Some(module_signature) = render_module_signature(db, target_module_id)
    {
        signature.push_str("\n\n");
        signature.push_str(&module_signature);
    }

    Some(signature)
}

fn render_decl_signature(db: &RootDb, decl_id: InContainer<DeclId>) -> Option<String> {
    let container = decl_id.cont_id.to_container(db);
    let decl = container.get(decl_id.value);
    decl.name.as_ref()?;

    match decl.parent {
        DeclaratorParent::PortDeclId(port_decl_id) => {
            let ContainerId::ModuleId(module_id) = decl_id.cont_id else {
                return None;
            };
            let module = db.module(module_id);
            let header = InModule::new(module_id, module.get(port_decl_id).header.clone())
                .display_source(db)
                .ok()?;
            let decl =
                InContainer::new(decl_id.cont_id, decl_id.value).display_signature(db).ok()?;
            Some(format!("{header} {decl}"))
        }
        DeclaratorParent::DeclarationId(parent) => {
            let declaration = container.get(parent);
            let prefix = render_declaration_prefix(db, decl_id.cont_id, declaration)?;
            let decl =
                InContainer::new(decl_id.cont_id, decl_id.value).display_signature(db).ok()?;
            let initializer = render_initializer(db, decl_id).unwrap_or_default();
            Some(format!("{prefix} {decl}{initializer}"))
        }
        DeclaratorParent::StmtId(_) => {
            let decl =
                InContainer::new(decl_id.cont_id, decl_id.value).display_signature(db).ok()?;
            let initializer = render_initializer(db, decl_id).unwrap_or_default();
            Some(format!("variable {decl}{initializer}"))
        }
    }
}

fn render_declaration_prefix(
    db: &RootDb,
    cont_id: ContainerId,
    declaration: &Declaration,
) -> Option<String> {
    let ty = render_data_ty(db, cont_id, declaration.ty()).unwrap_or_default();

    let prefix = match declaration {
        Declaration::DataDecl(data_decl) => {
            let mut prefix = String::new();
            if data_decl.const_kw {
                prefix.push_str("const ");
            }
            if data_decl.var_kw {
                prefix.push_str("var ");
            }
            prefix.push_str(&ty);
            prefix
        }
        Declaration::NetDecl(net_decl) => {
            let mut prefix = String::new();
            if let Some(kind) = net_decl.net_kind {
                prefix.push_str(&format!(
                    "{}{}",
                    kind.display_source(db).ok()?,
                    if ty.is_empty() { "" } else { " " }
                ));
            }
            prefix.push_str(&ty);
            prefix
        }
        Declaration::ParamDecl(_) => format!("parameter {ty}"),
        Declaration::GenvarDecl(_) => format!("genvar {ty}"),
        Declaration::SpecparamDecl(_) => {
            if ty.is_empty() {
                "specparam".to_string()
            } else {
                format!("specparam {ty}")
            }
        }
    };

    Some(prefix.trim().to_string())
}

fn render_initializer(db: &RootDb, decl_id: InContainer<DeclId>) -> Option<String> {
    let container = decl_id.cont_id.to_container(db);
    let decl = container.get(decl_id.value);
    let init = decl
        .initializer
        .map(|expr| InContainer::new(decl_id.cont_id, expr).display_source(db).ok())??;
    let mut rendered = format!(" = {init}");
    if let Some(second) = decl
        .secondary_initializer
        .and_then(|expr| InContainer::new(decl_id.cont_id, expr).display_source(db).ok())
    {
        rendered.push(':');
        rendered.push_str(&second);
    }
    Some(rendered)
}

fn render_data_ty(db: &RootDb, container: ContainerId, ty: DataTy) -> Option<String> {
    InContainer::new(container, ty).display_source(db).ok()
}

fn render_label_signature(db: &RootDb, origin: &DefinitionOrigin) -> Option<String> {
    let name = origin.name(db)?;
    let kind = match origin {
        DefinitionOrigin::Config(_) => "config",
        DefinitionOrigin::Library(_) => "library",
        DefinitionOrigin::Udp(_) => "primitive",
        DefinitionOrigin::BlockId(_) => "block",
        DefinitionOrigin::GenerateBlockId(_) => "generate",
        DefinitionOrigin::Instance(_) => "instance",
        DefinitionOrigin::Modport(_) => "modport",
        DefinitionOrigin::Stmt(_) => "statement",
        DefinitionOrigin::Typedef(_) => "typedef",
        DefinitionOrigin::ModuleId(_)
        | DefinitionOrigin::SubroutineId(_)
        | DefinitionOrigin::SubroutinePort(_)
        | DefinitionOrigin::NonAnsiPort(_)
        | DefinitionOrigin::Decl(_) => return None,
    };
    Some(format!("{kind} {name}"))
}

fn render_side_comments(sema: &Semantics<'_, RootDb>, origin: &DefinitionOrigin) -> Option<Markup> {
    let db = sema.db;
    let InFile { value: range, file_id } = origin.range(db)?;

    let parsed_file = sema.parse_file(file_id.file_id());
    let root = parsed_file.root()?;
    let elem = root.elem_at_exact_range(range)?;
    let mut offset = elem.text_range()?.end();

    loop {
        let mut cursor = root.walk();
        if !cursor.goto_first_tok_after(offset) {
            return None;
        }

        let tok = cursor.to_tok_with_parent()?;
        for (range, trivia) in tok.trivias_with_range() {
            if range.end() <= offset {
                continue;
            }

            if trivia.kind().is_eol() {
                return None;
            }

            if let Some(comment) = trivia.as_comment() {
                return Some(comment.to_string().into());
            }
        }

        let tok_range = tok.text_range()?;
        if tok_range.end() <= offset {
            return None;
        }
        offset = tok_range.end();
    }
}

fn render_containers(sema: &Semantics<RootDb>, origin: &DefinitionOrigin) -> Markup {
    // elaboration?
    let db = sema.db;
    let Some(InFile { value: range, .. }) = origin.range(db) else {
        return Markup::new();
    };
    let cont_id = origin.container_id(db);

    let mut containers = Vec::new();

    for cont_id in ContainerParent::start_from(db, cont_id) {
        let src_map = cont_id.to_container_src_map(db);

        if let Some(region_tree) = src_map.region_tree()
            && let Some(node) = region_tree.find(range.start())
        {
            for region in RegionParent::start_from(region_tree, node) {
                containers.push(format!("({})", region.name()));
            }
        }

        if !matches!(cont_id, ContainerId::HirFileId(_)) {
            if let Some(name) = cont_id.to_container(db).name() {
                containers.push(name.to_string());
            } else {
                containers.push(DEFAULT_NAME.to_string());
            }
        }
    }

    let mut ans = Markup::new();
    if containers.is_empty() {
        return ans;
    }
    ans.print("in ");
    ans.push_with_backticks(&containers.into_iter().rev().join(" > "));
    ans
}
