use std::sync::LazyLock;

use hir_def::{
    expr::data_ty::{BuiltinDataTy, DataTy},
    module::{
        ModuleId,
        port::{NonAnsiPort, PortDirection, Ports},
    },
    symbol::NameContext,
};
use hir_semantics::semantics::Semantics;
use hir_ty::db::TyDb;
use regex::{Regex, RegexBuilder};
use smallvec::SmallVec;
use utils::text_edit::TextRange;

use super::{SemaTokenCollector, SemaTokenTag};
use crate::{
    db::root_db::RootDb,
    module_resolution::resolve_port_metadata,
    semantic_tokens::{SemaToken, SemaTokenModifier, SemaTokenPort, check_range},
};

pub(super) fn collect_port(
    sema: &Semantics<'_, RootDb>,
    module_id: ModuleId,
    collector: &mut SemaTokenCollector,
) {
    if !collector.config.port() {
        return;
    }

    let db = sema.db;
    let module_scope = db.module_scope(module_id);
    let module = db.module_with_source_map(module_id);
    let body = db.module_body_with_source_map(module_id);

    match &module.ports {
        Ports::NonAnsi { ports, decls, .. } => {
            for (port_id, NonAnsiPort { refs, .. }) in ports.iter() {
                let Some(port_range) = module.source_range(sema.db, port_id) else {
                    continue;
                };
                check_range!(collector, port_range);
                let Some(refs) = refs.clone() else {
                    continue;
                };

                for ref_id in refs {
                    let _: Option<()> = try {
                        let name_range = module.source_name_range(sema.db, ref_id)?;
                        check_range!(collector, name_range);

                        let name = module.get(ref_id).ident.as_ref()?;
                        let def = module_scope.lookup(NameContext::Value, name).unique()?;
                        let origins = def.origins(db);
                        let (_, dir, ty) = resolve_port_metadata(db, &module, &body, &origins)?;
                        add_port_token(db, name, dir, ty, name_range, collector);
                    };
                }

                for (port_decl_id, port_decl) in decls.iter() {
                    let Some(port_decl_range) = module.source_range(sema.db, port_decl_id) else {
                        continue;
                    };
                    check_range!(collector, port_decl_range);

                    for decl_id in port_decl.decls.clone() {
                        let _: Option<()> = try {
                            let decl = body.get(decl_id);
                            let name_range = body.source_name_range(sema.db, decl_id)?;
                            check_range!(collector, name_range);

                            let name = decl.name.as_ref()?;
                            let def = module_scope.lookup(NameContext::Value, name).unique()?;
                            let origins = def.origins(db);
                            let (_, dir, ty) = resolve_port_metadata(db, &module, &body, &origins)?;
                            add_port_token(db, name, dir, ty, name_range, collector);
                        };
                    }
                }
            }
        }
        Ports::Ansi(port_decls) => {
            for (port_decl_id, port_decl) in port_decls.iter() {
                let Some(port_decl_range) = module.source_range(sema.db, port_decl_id) else {
                    continue;
                };
                check_range!(collector, port_decl_range);

                for decl_id in port_decl.decls.clone() {
                    let _: Option<()> = try {
                        let decl = body.get(decl_id);
                        let name_range = body.source_name_range(sema.db, decl_id)?;
                        check_range!(collector, name_range);

                        let name = decl.name.as_ref()?;
                        let header = &port_decl.header;
                        let (dir, ty) = (Some(header.dir()), header.ty());
                        add_port_token(db, name, dir, ty, name_range, collector);
                    };
                }
            }
        }
    }
}

pub(super) fn add_port_token(
    _db: &dyn TyDb,
    name: &str,
    dir: Option<PortDirection>,
    ty: DataTy,
    range: TextRange,
    collector: &mut SemaTokenCollector,
) {
    let Some(tag) = port_tag(ty, name, collector) else {
        return;
    };

    let mods = if collector.config.port.io
        && let Some(dir) = dir
    {
        match dir {
            PortDirection::Input => SemaTokenModifier::READ,
            PortDirection::Output => SemaTokenModifier::WRITE,
            PortDirection::Ref => SemaTokenModifier::REF,
            PortDirection::Inout => SemaTokenModifier::READ | SemaTokenModifier::WRITE,
        }
    } else {
        SemaTokenModifier::empty()
    };

    collector.tokens.add(SemaToken { range, tag, mods });
}

fn port_tag(ty: DataTy, name: &str, collector: &mut SemaTokenCollector) -> Option<SemaTokenTag> {
    static CLK_RE: LazyLock<Option<Regex>> = LazyLock::new(|| {
        RegexBuilder::new(r"(clock|clk|tck)\d*$").case_insensitive(true).build().ok()
    });

    static RST_RE: LazyLock<Option<Regex>> = LazyLock::new(|| {
        // Rust regex does not support look-around; keep this approximation in sync with
        // tests.
        RegexBuilder::new(r"reset|(^rst|[^aeiou]rst)($|[^o]|o[^r]|or[^e]|[^a]|a[^r]|ar[^t])")
            .case_insensitive(true)
            .build()
            .ok()
    });

    if !collector.config.port.clk_rst {
        return Some(SemaTokenTag::Port(SemaTokenPort::Others));
    }

    // check if the port is a 1-bit vector
    let DataTy::Builtin(tyid) = ty else {
        return Some(SemaTokenTag::Port(SemaTokenPort::Others));
    };
    let BuiltinDataTy::Vector { dimensions, .. } = tyid.get() else {
        return Some(SemaTokenTag::Port(SemaTokenPort::Others));
    };
    if !dimensions.is_empty() {
        return Some(SemaTokenTag::Port(SemaTokenPort::Others));
    }

    let segments = split_name(name);
    if segments.iter().any(|segment| CLK_RE.as_ref().is_some_and(|regex| regex.is_match(segment))) {
        Some(SemaTokenTag::Port(SemaTokenPort::Clk))
    } else if segments
        .iter()
        .any(|segment| RST_RE.as_ref().is_some_and(|regex| regex.is_match(segment)))
    {
        Some(SemaTokenTag::Port(SemaTokenPort::Rst))
    } else {
        Some(SemaTokenTag::Port(SemaTokenPort::Others))
    }
}

// split by underscore and case changes
fn split_name(name: &str) -> SmallVec<[&str; 4]> {
    let mut segments = SmallVec::new();

    for name in name.split('_') {
        let mut last_pos = 0;
        for ((i, ch), nxt) in name.chars().enumerate().zip(name.chars().skip(1)) {
            if ch.is_lowercase() && nxt.is_uppercase() {
                segments.push(&name[last_pos..=i]);
                last_pos = i + 1;
            }
        }
        if last_pos < name.len() {
            segments.push(&name[last_pos..]);
        }
    }

    segments
}
