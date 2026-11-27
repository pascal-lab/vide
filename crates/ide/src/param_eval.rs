//! Parameter value evaluation wired to the workspace layer.
//!
//! [`instantiation_param_overrides`] turns a module instantiation's `#(...)`
//! list into an evaluated override map for the target module: named
//! assignments resolve through workspace module-name resolution, ordered
//! assignments map onto overridable parameter ports positionally, and every
//! override expression is evaluated in the instantiating module's own scope.
//! The map feeds `hir_ty::const_eval`, so dependent parameters (for example
//! `localparam DEPTH = WIDTH * 2`) re-evaluate against the overridden values.

use hir_def::{
    body::{Body, BodyItem},
    container::OwnerRef,
    declaration::Declaration,
    expr::{ExprId, declarator::DeclId},
    module::instantiation::{InstanceId, Instantiation, ParamAssign},
    owner::OwnerId,
    source_map::Lowered,
};
use hir_ty::const_eval::{self, ConstValue, ParamOverrides};
use utils::text_edit::TextRange;
use vfs::FileId;

use crate::{
    db::root_db::RootDb,
    module_resolution::{resolve_hir_instantiation_target, resolve_named_param_in_module},
};

/// The default (unoverridden) value of a parameter's initializer.
pub(crate) fn param_value(db: &RootDb, decl: OwnerRef<DeclId>) -> Option<ConstValue> {
    let overrides = ParamOverrides::default();
    effective_param_value(db, &overrides, decl)
}

/// The value a parameter takes under `overrides`: an explicit override wins,
/// otherwise the initializer is evaluated with the overrides in effect.
pub(crate) fn effective_param_value(
    db: &RootDb,
    overrides: &ParamOverrides,
    decl: OwnerRef<DeclId>,
) -> Option<ConstValue> {
    if let Some(value) = overrides.get(&decl) {
        return Some(value.clone());
    }
    const_eval::eval_param_initializer(db, decl, overrides)
}

/// Evaluates every parameter override passed by `instantiation` (inside
/// `parent_module`) to its target module.
pub(crate) fn instantiation_param_overrides(
    db: &RootDb,
    parent_module: OwnerId,
    from_file: FileId,
    instantiation: &Instantiation,
) -> ParamOverrides {
    let mut overrides = ParamOverrides::default();
    let Some(target_module_id) = resolve_hir_instantiation_target(db, from_file, instantiation)
    else {
        return overrides;
    };

    let body = db.body_with_source_map(parent_module);
    for (idx, assign_id) in instantiation.param_assigns.iter().enumerate() {
        match body.get(*assign_id) {
            ParamAssign::Named(Some(name), Some(expr)) => {
                let Some(def_id) =
                    resolve_named_param_in_module(db, target_module_id, name).unique()
                else {
                    continue;
                };
                let Some(decl) = def_id.primary_origin(db).as_decl(db) else { continue };
                if let Some(value) = eval_in_module(db, parent_module, *expr) {
                    overrides.insert(decl, value);
                }
            }
            ParamAssign::Ordered(expr) => {
                let target_body = db.body_with_source_map(target_module_id);
                let Some(decl_id) =
                    hir_def::module::overridable_param_id_by_idx(target_body.data_ref(), idx)
                else {
                    continue;
                };
                if let Some(value) = eval_in_module(db, parent_module, *expr) {
                    overrides.insert(OwnerRef::new(target_module_id, decl_id), value);
                }
            }
            ParamAssign::Named(None, _) | ParamAssign::Named(Some(_), None) => {}
        }
    }

    overrides
}

/// Evaluates an expression in `module_id`'s scope with no overrides in effect.
pub(crate) fn eval_in_module(db: &RootDb, module_id: OwnerId, expr: ExprId) -> Option<ConstValue> {
    let overrides = ParamOverrides::default();
    const_eval::eval_const_expr(db, OwnerRef::new(module_id, expr), &overrides)
}

/// The overrides passed by the instantiation that owns `instance`.
pub(crate) fn overrides_for_instance(
    db: &RootDb,
    instance: OwnerRef<InstanceId>,
) -> ParamOverrides {
    let body = db.body_with_source_map(instance.cont_id);
    let instantiation = body.get(body.get(instance.value).parent);
    let Some(from_file) = instance.cont_id.file(db).source_file_id(db) else {
        return ParamOverrides::default();
    };
    instantiation_param_overrides(db, instance.cont_id, from_file, instantiation)
}

/// Finds the instantiation whose source range covers `range`, searching the
/// owner's body and recursing into generate regions and blocks. Returns the
/// owner whose body holds the instantiation - the scope in which its override
/// expressions are evaluated.
pub(crate) fn instantiation_covering(
    db: &RootDb,
    owner: OwnerId,
    range: TextRange,
) -> Option<(OwnerId, Instantiation)> {
    let body = db.body_with_source_map(owner);
    find_instantiation(db, owner, &body, range)
}

fn find_instantiation(
    db: &RootDb,
    owner: OwnerId,
    body: &Lowered<Body>,
    range: TextRange,
) -> Option<(OwnerId, Instantiation)> {
    for item in &body.data_ref().items {
        let found = match item {
            BodyItem::InstantiationId(id) => body
                .source_range(db, *id)
                .is_some_and(|instantiation_range| instantiation_range.contains_range(range))
                .then(|| (owner, body.get(*id).clone())),
            BodyItem::GenerateRegionId(region_id) => {
                let region = body.get(*region_id);
                region
                    .items
                    .iter()
                    .find_map(|item| find_instantiation_item(db, owner, body, item, range))
            }
            BodyItem::GenerateBlockOwner(block_owner) => {
                let block_body = db.body_with_source_map(*block_owner);
                find_instantiation(db, *block_owner, &block_body, range)
            }
            _ => None,
        };
        if found.is_some() {
            return found;
        }
    }
    None
}

fn find_instantiation_item(
    db: &RootDb,
    owner: OwnerId,
    body: &Lowered<Body>,
    item: &BodyItem,
    range: TextRange,
) -> Option<(OwnerId, Instantiation)> {
    match item {
        BodyItem::InstantiationId(id) => body
            .source_range(db, *id)
            .is_some_and(|instantiation_range| instantiation_range.contains_range(range))
            .then(|| (owner, body.get(*id).clone())),
        BodyItem::GenerateBlockOwner(block_owner) => {
            let block_body = db.body_with_source_map(*block_owner);
            find_instantiation(db, *block_owner, &block_body, range)
        }
        _ => None,
    }
}

/// One target-module parameter with its effective value under an
/// instantiation's overrides.
pub(crate) struct EffectiveParam {
    pub name: String,
    pub keyword: &'static str,
    pub overridden: bool,
    pub value: Option<ConstValue>,
    pub initializer: Option<ExprId>,
}

/// Every value parameter of the target module in declaration order, with
/// effective values under `overrides`.
pub(crate) fn effective_params(
    db: &RootDb,
    target_module: OwnerId,
    overrides: &ParamOverrides,
) -> Vec<EffectiveParam> {
    let body = db.body_with_source_map(target_module);
    let mut params = Vec::new();
    for (_, declaration) in body.data_ref().declarations.iter() {
        let Declaration::ParamDecl(param_decl) = declaration else { continue };
        for decl_id in param_decl.decls.clone() {
            let declarator = body.data_ref().declarator(decl_id);
            let Some(name) = declarator.name.clone() else { continue };
            let decl = OwnerRef::new(target_module, decl_id);
            params.push(EffectiveParam {
                name: name.to_string(),
                keyword: param_decl.kind.keyword(),
                overridden: overrides.contains_key(&decl),
                value: effective_param_value(db, overrides, decl),
                initializer: declarator.initializer,
            });
        }
    }
    params
}

#[cfg(test)]
mod tests {
    use base_db::{change::Change, source_root::SourceRoot};
    use hir_def::{
        body::BodyItem, container::InFile, declaration::Declaration, expr::declarator::DeclId,
        module::instantiation::Instantiation, symbol::NameContext,
    };
    use preproc_expand::file::HirFileId;
    use vfs::{ChangedFile, FileId, FileSet, VfsPath};

    use super::*;

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
        db.apply_change(change);
        (db, file_id)
    }

    fn module_owner(db: &RootDb, file_id: FileId, name: &str) -> OwnerId {
        let file_owner = db.owner_table(HirFileId::File(file_id)).file_owner().unwrap();
        db.body_with_source_map(file_owner)
            .data_ref()
            .items
            .iter()
            .filter_map(|item| match item {
                BodyItem::ModuleOwner(owner) if db.body(*owner).name.as_deref() == Some(name) => {
                    Some(*owner)
                }
                _ => None,
            })
            .next()
            .unwrap_or_else(|| panic!("module {name} not found"))
    }

    fn param_decl(db: &RootDb, module: OwnerId, name: &str) -> OwnerRef<DeclId> {
        let body = db.body_with_source_map(module);
        for (_, declaration) in body.data_ref().declarations.iter() {
            let Declaration::ParamDecl(param_decl) = declaration else { continue };
            for decl_id in param_decl.decls.clone() {
                if body.data_ref().declarator(decl_id).name.as_deref() == Some(name) {
                    return OwnerRef::new(module, decl_id);
                }
            }
        }
        panic!("parameter {name} not found in module");
    }

    fn value_of(db: &RootDb, module: OwnerId, name: &str) -> Option<ConstValue> {
        param_value(db, param_decl(db, module, name))
    }

    fn instantiation_of(db: &RootDb, module: OwnerId) -> Instantiation {
        let body = db.body_with_source_map(module);
        for item in &body.data_ref().items {
            if let BodyItem::InstantiationId(id) = item {
                return body.get(*id).clone();
            }
        }
        panic!("no instantiation found in module");
    }

    fn eval_int(value: &ConstValue) -> i128 {
        match value {
            ConstValue::Int(int) if !int.has_unknown => int.value,
            other => panic!("expected an integer value, got {other:?}"),
        }
    }

    fn eval_real(value: &ConstValue) -> f64 {
        match value {
            ConstValue::Real(real) => *real,
            other => panic!("expected a real value, got {other:?}"),
        }
    }

    #[test]
    fn algebraic_expressions_fold() {
        let text = r#"
module m;
  localparam A = 4 * 3 + 2;
  localparam B = (1 << 4) - 1;
  localparam C = 2 ** 10;
  localparam D = 17 / 5;
  localparam E = 17 % 5;
  localparam F = (A > 10) ? 1 : 0;
  localparam G = {4{3'b101}};
  localparam H = -A;
  localparam I = 1779;
  localparam int J = I[3];
  localparam int K = I[7:4];
  localparam logic [9:0] L = I[9:0];
endmodule
"#;
        let (db, file_id) = db_with_file(text);
        let m = module_owner(&db, file_id, "m");

        assert_eq!(eval_int(&value_of(&db, m, "A").unwrap()), 14);
        assert_eq!(eval_int(&value_of(&db, m, "B").unwrap()), 15);
        assert_eq!(eval_int(&value_of(&db, m, "C").unwrap()), 1024);
        assert_eq!(eval_int(&value_of(&db, m, "D").unwrap()), 3);
        assert_eq!(eval_int(&value_of(&db, m, "E").unwrap()), 2);
        assert_eq!(eval_int(&value_of(&db, m, "F").unwrap()), 1);
        assert_eq!(eval_int(&value_of(&db, m, "G").unwrap()), 0b101101101101);
        assert_eq!(eval_int(&value_of(&db, m, "H").unwrap()), -14);
        // Bit and range selects read the value's two's-complement bits.
        // 1779 = 'b110_1111_0011
        assert_eq!(eval_int(&value_of(&db, m, "I").unwrap()), 1779);
        assert_eq!(eval_int(&value_of(&db, m, "J").unwrap()), 0);
        assert_eq!(eval_int(&value_of(&db, m, "K").unwrap()), 15);
        assert_eq!(eval_int(&value_of(&db, m, "L").unwrap()), 755);
    }

    #[test]
    fn system_functions_fold() {
        let text = r#"
module m;
  localparam A = $clog2(100);
  localparam B = $clog2(64);
  localparam C = $ceil(3.2);
  localparam D = $floor(3.8);
  localparam E = $pow(2, 8);
  localparam F = $sqrt(16.0);
  localparam G = $countones(8'b1011_1100);
  localparam H = $signed(-4);
endmodule
"#;
        let (db, file_id) = db_with_file(text);
        let m = module_owner(&db, file_id, "m");

        assert_eq!(eval_int(&value_of(&db, m, "A").unwrap()), 7);
        assert_eq!(eval_int(&value_of(&db, m, "B").unwrap()), 6);
        assert_eq!(eval_real(&value_of(&db, m, "C").unwrap()), 4.0);
        assert_eq!(eval_real(&value_of(&db, m, "D").unwrap()), 3.0);
        assert_eq!(eval_real(&value_of(&db, m, "E").unwrap()), 256.0);
        assert_eq!(eval_real(&value_of(&db, m, "F").unwrap()), 4.0);
        assert_eq!(eval_int(&value_of(&db, m, "G").unwrap()), 5);
        assert_eq!(eval_int(&value_of(&db, m, "H").unwrap()), -4);
    }

    #[test]
    fn bits_covers_structs_unions_and_arrays() {
        let text = r#"
module m;
  typedef struct packed {
    logic [7:0] a;
    logic [3:0] b;
  } packed_t;
  typedef struct {
    logic [1:0] x;
    int y;
  } unpacked_t;
  typedef union packed {
    logic [7:0] narrow;
    logic [15:0] wide;
  } union_t;

  localparam int A = $bits(packed_t);
  localparam int B = $bits(unpacked_t);
  localparam int C = $bits(union_t);
  localparam packed_t P = '0;
  localparam int D = $bits(P);
  int E [4];
  localparam int F = $bits(E);
  localparam int G = $size(E);
  localparam int H = $dimensions(E);
  parameter WIDTH = 8;
  localparam int I = $bits(WIDTH);
endmodule
"#;
        let (db, file_id) = db_with_file(text);
        let m = module_owner(&db, file_id, "m");

        assert_eq!(eval_int(&value_of(&db, m, "A").unwrap()), 12);
        assert_eq!(eval_int(&value_of(&db, m, "B").unwrap()), 34);
        assert_eq!(eval_int(&value_of(&db, m, "C").unwrap()), 16);
        assert_eq!(eval_int(&value_of(&db, m, "D").unwrap()), 12);
        assert_eq!(eval_int(&value_of(&db, m, "F").unwrap()), 128);
        assert_eq!(eval_int(&value_of(&db, m, "G").unwrap()), 4);
        assert_eq!(eval_int(&value_of(&db, m, "H").unwrap()), 2);
        // An untyped parameter takes the width of its assignment expression.
        assert_eq!(eval_int(&value_of(&db, m, "I").unwrap()), 32);
    }

    #[test]
    fn enum_members_fold_with_implicit_values() {
        let text = r#"
module m;
  typedef enum { IDLE, BUSY = 5, DONE } state_t;
  localparam state_t A = DONE;
  localparam int B = BUSY;
endmodule
"#;
        let (db, file_id) = db_with_file(text);
        let m = module_owner(&db, file_id, "m");

        assert_eq!(eval_int(&value_of(&db, m, "A").unwrap()), 6);
        assert_eq!(eval_int(&value_of(&db, m, "B").unwrap()), 5);
    }

    #[test]
    fn package_scoped_parameters_resolve() {
        let text = r#"
package p;
  parameter WIDTH = 4;
endpackage

module m;
  localparam A = p::WIDTH * 2;
  localparam int B = $bits(p::WIDTH);
endmodule
"#;
        let (db, file_id) = db_with_file(text);
        let m = module_owner(&db, file_id, "m");

        assert_eq!(eval_int(&value_of(&db, m, "A").unwrap()), 8);
        assert_eq!(eval_int(&value_of(&db, m, "B").unwrap()), 32);
    }

    #[test]
    fn reference_cycles_do_not_recurse() {
        let text = r#"
module m;
  localparam X = Y;
  localparam Y = X;
endmodule
"#;
        let (db, file_id) = db_with_file(text);
        let m = module_owner(&db, file_id, "m");

        assert!(value_of(&db, m, "X").is_none());
    }

    #[test]
    fn named_overrides_propagate_to_dependent_params() {
        let text = r#"
module child #(parameter WIDTH = 2, parameter int UNSIGNED = 1);
  localparam DEPTH = WIDTH * 2;
  localparam TWICE = DEPTH + WIDTH;
endmodule

module top;
  localparam SCALE = 2;
  child #(.WIDTH(4 * SCALE), .UNSIGNED(0)) u();
endmodule
"#;
        let (db, file_id) = db_with_file(text);
        let top = module_owner(&db, file_id, "top");
        let child = module_owner(&db, file_id, "child");

        let instantiation = instantiation_of(&db, top);
        let overrides = instantiation_param_overrides(&db, top, file_id, &instantiation);
        assert_eq!(overrides.len(), 2);

        let width = param_decl(&db, child, "WIDTH");
        let depth = param_decl(&db, child, "DEPTH");
        let twice = param_decl(&db, child, "TWICE");
        let unsigned = param_decl(&db, child, "UNSIGNED");

        assert_eq!(eval_int(&effective_param_value(&db, &overrides, width).unwrap()), 8);
        assert_eq!(eval_int(&effective_param_value(&db, &overrides, depth).unwrap()), 16);
        assert_eq!(eval_int(&effective_param_value(&db, &overrides, twice).unwrap()), 24);
        assert_eq!(eval_int(&effective_param_value(&db, &overrides, unsigned).unwrap()), 0);

        // Defaults stay untouched without overrides.
        assert_eq!(eval_int(&param_value(&db, width).unwrap()), 2);
        assert_eq!(eval_int(&param_value(&db, depth).unwrap()), 4);
    }

    #[test]
    fn ordered_overrides_map_to_overridable_positions() {
        let text = r#"
module child #(localparam L = 1, parameter P = 2);
  localparam DEPTH = P + 1;
endmodule

module top;
  child #(8) u();
endmodule
"#;
        let (db, file_id) = db_with_file(text);
        let top = module_owner(&db, file_id, "top");
        let child = module_owner(&db, file_id, "child");

        let instantiation = instantiation_of(&db, top);
        let overrides = instantiation_param_overrides(&db, top, file_id, &instantiation);

        // The ordered assignment skips the localparam and targets `P`.
        let p = param_decl(&db, child, "P");
        let depth = param_decl(&db, child, "DEPTH");
        assert_eq!(eval_int(&effective_param_value(&db, &overrides, p).unwrap()), 8);
        assert_eq!(eval_int(&effective_param_value(&db, &overrides, depth).unwrap()), 9);
    }

    #[test]
    fn unresolved_expressions_stay_un_evaluated() {
        let text = r#"
module m;
  localparam A = missing_function(3);
  localparam B = some_variable;
endmodule
"#;
        let (db, file_id) = db_with_file(text);
        let m = module_owner(&db, file_id, "m");

        assert!(value_of(&db, m, "A").is_none());
        assert!(value_of(&db, m, "B").is_none());
    }

    #[test]
    fn scope_lookup_finds_parameters_by_name() {
        let text = "module m; localparam A = 1; endmodule\n";
        let (db, file_id) = db_with_file(text);
        let m = module_owner(&db, file_id, "m");
        let ident = hir_def::Ident::from("A");
        let defs = db.scope(m).lookup(NameContext::Value, &ident);
        assert!(defs.into_candidates().len() >= 1);
    }

    /// Full-project probe over a real RTL tree. Set `VIDE_PARAM_PROBE_DIR` and
    /// run with `cargo test -p ide --lib param_eval::tests::probe -- --ignored
    /// --nocapture`; evaluates every parameter default, every instantiation's
    /// effective parameters, and renders the real hover for each, so panics or
    /// display/value mismatches on real code surface as test failures.
    #[test]
    #[ignore = "manual probe over a real source tree"]
    fn probe_real_project_parameters() {
        use std::{path::PathBuf, time::Instant};

        use base_db::source_root::SourceRoot;
        use hir_def::{module::instantiation::InstantiationId, owner::OwnerId};
        use utils::line_index::TextSize;
        use vfs::VfsPath;

        use crate::{hover::hover, range::FilePosition};

        fn name_range(
            db: &RootDb,
            def_id: hir_def::def_id::DefId,
        ) -> Option<utils::text_edit::TextRange> {
            let InFile { value: range, file_id } = def_id.name_range(db)?;
            file_id.as_file()?;
            Some(range)
        }

        fn collect_instantiation_ids(
            db: &RootDb,
            owner: OwnerId,
            body: &Lowered<hir_def::body::Body>,
            items: &[BodyItem],
            out: &mut Vec<(OwnerId, InstantiationId)>,
        ) {
            for item in items {
                match item {
                    BodyItem::InstantiationId(id) => out.push((owner, *id)),
                    BodyItem::GenerateRegionId(region_id) => {
                        let region = body.get(*region_id);
                        collect_instantiation_ids(db, owner, body, &region.items.clone(), out);
                    }
                    BodyItem::GenerateBlockOwner(block_owner) => {
                        let block_body = db.body_with_source_map(*block_owner);
                        collect_instantiation_ids(
                            db,
                            *block_owner,
                            &block_body,
                            &block_body.data_ref().items.clone(),
                            out,
                        );
                    }
                    _ => {}
                }
            }
        }

        let root =
            PathBuf::from(std::env::var("VIDE_PARAM_PROBE_DIR").expect("set VIDE_PARAM_PROBE_DIR"));

        let mut file_set = FileSet::default();
        let mut files = Vec::new();
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if let Some(ext) = path.extension().and_then(|ext| ext.to_str())
                    && matches!(ext, "v" | "sv" | "vh" | "svh" | "svi")
                {
                    let Ok(text) = std::fs::read_to_string(&path) else { continue };
                    let rel = path
                        .strip_prefix(&root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    let file_id = FileId::from_raw(files.len() as u32);
                    file_set.insert(file_id, VfsPath::new_virtual_path(format!("/proj{rel}")));
                    files.push((file_id, text));
                }
            }
        }
        assert!(!files.is_empty(), "no Verilog sources found under {}", root.display());

        let mut change = Change::new();
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        for (file_id, text) in &files {
            change.add_changed_file(ChangedFile::create(*file_id, text.as_str()));
        }
        let mut db = RootDb::new(None);
        db.apply_change(change);
        let db = &db;

        let started = Instant::now();
        let mut evaluated = 0usize;
        let mut unevaluated = 0usize;
        let mut instantiations = 0usize;
        let mut overrides_total = 0usize;
        let mut hovers = 0usize;

        for (file_id, _text) in &files {
            let file_owner = db.owner_table(HirFileId::File(*file_id)).file_owner().unwrap();
            for item in db.body_with_source_map(file_owner).data_ref().items.clone() {
                let BodyItem::ModuleOwner(module) = item else { continue };
                let Some(module_name) = db.body(module).name.clone() else { continue };
                let body = db.body_with_source_map(module);

                let mut param_lines = Vec::new();
                for (_, declaration) in body.data_ref().declarations.iter() {
                    let Declaration::ParamDecl(param_decl) = declaration else { continue };
                    for decl_id in param_decl.decls.clone() {
                        let declarator = body.data_ref().declarator(decl_id);
                        let Some(name) = declarator.name.clone() else { continue };
                        let decl = OwnerRef::new(module, decl_id);
                        let value = param_value(db, decl);
                        match value {
                            Some(_) => evaluated += 1,
                            None => unevaluated += 1,
                        }

                        // Render the real hover at the parameter's name so the
                        // display path is exercised on real code as well; a
                        // value must appear in the rendered markup.
                        let def_id = hir_def::def_id::DefId::from_source(db, decl);
                        if let Some(range) = name_range(db, def_id) {
                            let midpoint =
                                range.start() + TextSize::new(u32::from(range.len()) / 2);
                            if let Some(markup) =
                                hover(db, FilePosition { file_id: *file_id, offset: midpoint })
                            {
                                hovers += 1;
                                if let Some(value) = &value {
                                    let expected = format!("value: `{value}`");
                                    assert!(
                                        markup.info.as_str().contains(&expected),
                                        "hover for {module_name}.{name} rendered without \
                                         `{expected}`:\n{}",
                                        markup.info.as_str()
                                    );
                                }
                            }
                        }

                        param_lines.push(match value {
                            Some(value) => format!("  {name} = {value}"),
                            None => format!("  {name} = <unevaluated>"),
                        });
                    }
                }

                let mut instantiation_ids = Vec::new();
                collect_instantiation_ids(
                    db,
                    module,
                    &body,
                    &body.data_ref().items.clone(),
                    &mut instantiation_ids,
                );
                let mut instance_lines = Vec::new();
                for (owner, instantiation_id) in &instantiation_ids {
                    let owner_body = db.body_with_source_map(*owner);
                    let instantiation = owner_body.get(*instantiation_id);
                    let Some(target) =
                        resolve_hir_instantiation_target(db, *file_id, instantiation)
                    else {
                        continue;
                    };
                    for instance_id in instantiation.instances.iter() {
                        let instance_ref = OwnerRef::new(*owner, *instance_id);
                        let overrides = overrides_for_instance(db, instance_ref);
                        instantiations += 1;
                        overrides_total += overrides.len();
                        let params = effective_params(db, target, &overrides);
                        let summary = params
                            .iter()
                            .map(|param| match &param.value {
                                Some(value) if param.overridden => {
                                    format!("{}={value}*", param.name)
                                }
                                Some(value) => format!("{}={value}", param.name),
                                None => param.name.clone(),
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        instance_lines.push(format!(
                            "  inst {:?} of {} [{} overrides]: {summary}",
                            owner_body.get(*instance_id).name,
                            db.body(target).name.as_deref().unwrap_or("?"),
                            overrides.len(),
                        ));

                        // Hover the instance name to exercise the effective
                        // parameters rendering end to end.
                        let def_id = hir_def::def_id::DefId::from_source(db, instance_ref);
                        if let Some(range) = name_range(db, def_id) {
                            let midpoint =
                                range.start() + TextSize::new(u32::from(range.len()) / 2);
                            let _ = hover(db, FilePosition { file_id: *file_id, offset: midpoint });
                            hovers += 1;
                        }
                    }
                }

                if !param_lines.is_empty() || !instance_lines.is_empty() {
                    println!(
                        "[{module_name}] ({} params, {} insts)",
                        param_lines.len(),
                        instance_lines.len()
                    );
                    for line in param_lines.iter().chain(instance_lines.iter()) {
                        println!("{line}");
                    }
                }
            }
        }

        println!(
            "probe: {} files, {} params ({} evaluated, {} unevaluated), {} instantiations \
             ({} overrides), {} hovers, elapsed {:?}",
            files.len(),
            evaluated + unevaluated,
            evaluated,
            unevaluated,
            instantiations,
            overrides_total,
            hovers,
            started.elapsed(),
        );
    }
}
