//! Constant evaluation for parameter-facing expressions.
//!
//! The evaluator walks the lowered `hir_def::expr::Expr` tree and folds it into
//! a [`ConstValue`]. Identifiers resolve through the ordinary name-resolution
//! path (`pathres::resolve_name_at`), so references to other parameters,
//! genvars, enum members, and package-scoped parameters participate in the
//! evaluation. An optional override map substitutes instantiation-site
//! parameter values before a module's own initializers are consulted.
//!
//! Evaluation is intentionally conservative: anything that cannot be folded
//! faithfully (four-state contamination, bit patterns wider than 127 bits,
//! unresolvable names, unsupported constructs) yields `None`, and callers fall
//! back to displaying the raw source text instead of a value.

use std::fmt;

use hir_def::{
    Ident,
    aggregate::{EnumId, StructId, StructKind},
    container::{InFile, OwnerRef},
    def_id::DefId,
    expr::{
        Arg, BinaryOp, Expr, ExprId, PropertyExpr, Selector, SequenceExpr, UnaryOp,
        data_ty::{BuiltinDataTy, DataTy, Dimension, IntKind, Real},
        declarator::DeclId,
    },
    literal::Literal,
    owner::OwnerId,
    pathres::{NameRef, RefKind, descend_scope, resolve_name_at},
    symbol::{DefKind, DefOriginLoc, NameContext, Resolution},
};
use rustc_hash::{FxHashMap, FxHashSet};
use syntax::value::{Bit, SVInt};

use crate::{
    db::TyDb,
    infer::{data_ty_of_decl, normalize_data_ty, normalize_data_ty_with_owner},
    ty::Ty,
};

/// Overrides for a target module's parameters, keyed by the parameter's
/// declaring declarator. Values are already evaluated in the instantiating
/// context.
pub type ParamOverrides = FxHashMap<OwnerRef<DeclId>, ConstValue>;

/// Bit-pattern width above which the evaluator refuses to fold, keeping every
/// unsigned pattern representable in an `i128`.
const MAX_BITS: u32 = 127;

/// Recursion guard for pathological expression trees.
const MAX_DEPTH: u32 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstInt {
    /// Mathematical value (sign-extended beyond the declared width).
    pub value: i128,
    /// Declared or self-determined width; display information only.
    pub width: u32,
    pub signed: bool,
    /// Four-state contamination: the numeric value is meaningless.
    pub has_unknown: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstValue {
    Int(ConstInt),
    Real(f64),
    Str(Box<str>),
}

impl ConstValue {
    pub fn as_int(&self) -> Option<ConstInt> {
        match self {
            ConstValue::Int(int) => Some(*int),
            _ => None,
        }
    }

    fn truthy(&self) -> Option<bool> {
        match self {
            ConstValue::Int(int) if int.has_unknown => None,
            ConstValue::Int(int) => Some(int.value != 0),
            ConstValue::Real(value) => Some(*value != 0.0),
            ConstValue::Str(text) => Some(!text.is_empty()),
        }
    }

    fn promote_real(&self) -> Option<f64> {
        match self {
            ConstValue::Int(int) if !int.has_unknown => Some(int.value as f64),
            ConstValue::Real(value) => Some(*value),
            _ => None,
        }
    }
}

impl fmt::Display for ConstInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.has_unknown {
            return f.write_str("x");
        }
        write!(f, "{}", self.value)
    }
}

impl fmt::Display for ConstValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConstValue::Int(int) => write!(f, "{int}"),
            ConstValue::Real(value) => write!(f, "{value:?}"),
            ConstValue::Str(text) => write!(f, "\"{text}\""),
        }
    }
}

/// Evaluates a constant expression. Returns `None` when the expression cannot
/// be folded faithfully.
pub fn eval_const_expr(
    db: &dyn TyDb,
    expr: OwnerRef<ExprId>,
    overrides: &ParamOverrides,
) -> Option<ConstValue> {
    let mut ctx = EvalCtx { db, overrides, seen: FxHashSet::default(), depth: 0 };
    ctx.eval_expr(expr)
}

/// Evaluates the initializer of a declaration (typically a parameter) with the
/// given overrides in effect.
pub fn eval_param_initializer(
    db: &dyn TyDb,
    decl: OwnerRef<DeclId>,
    overrides: &ParamOverrides,
) -> Option<ConstValue> {
    let initializer = decl.cont_id.data(db).declarator(decl.value).initializer?;
    eval_const_expr(db, OwnerRef::new(decl.cont_id, initializer), overrides)
}

struct EvalCtx<'a> {
    db: &'a dyn TyDb,
    overrides: &'a ParamOverrides,
    seen: FxHashSet<OwnerRef<DeclId>>,
    depth: u32,
}

impl EvalCtx<'_> {
    fn eval_expr(&mut self, expr: OwnerRef<ExprId>) -> Option<ConstValue> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return None;
        }
        let result = self.eval_expr_inner(expr);
        self.depth -= 1;
        result
    }

    fn eval_expr_inner(&mut self, expr: OwnerRef<ExprId>) -> Option<ConstValue> {
        let data = expr.cont_id.data(self.db);
        match data.expr(expr.value) {
            Expr::Literal(literal) => eval_literal(literal),
            Expr::Ident(ident) => self.eval_ident(expr, ident),
            Expr::Field { receiver, field } => self.eval_scoped_field(expr, *receiver, field),
            Expr::Binary { op, lhs, rhs } => {
                let left = self.eval_expr(expr.with_value(*lhs))?;
                let right = self.eval_expr(expr.with_value(*rhs))?;
                apply_binary(*op, &left, &right)
            }
            Expr::Unary { op, expr: operand } => {
                let value = self.eval_expr(expr.with_value(*operand))?;
                apply_unary(*op, &value)
            }
            Expr::Cond { pred, true_expr, false_expr } => {
                let condition = self.eval_expr(expr.with_value(*pred))?;
                let branch = if condition.truthy()? { *true_expr } else { *false_expr };
                self.eval_expr(expr.with_value(branch))
            }
            Expr::MinTypMax { typ, .. } => self.eval_expr(expr.with_value(*typ)),
            Expr::Concat(items) => self.eval_concat(expr, items),
            Expr::MultiConcat { rep, concat } => self.eval_multi_concat(expr, *rep, concat),
            Expr::Cast { ty, expr: inner } => {
                let value = self.eval_expr(expr.with_value(*inner))?;
                self.apply_cast(expr.cont_id, ty, value)
            }
            Expr::SignedCast { signed, expr: inner } => {
                let value = self.eval_expr(expr.with_value(*inner))?;
                match value {
                    ConstValue::Int(int) => {
                        Some(ConstValue::Int(ConstInt { signed: *signed, ..int }))
                    }
                    _ => None,
                }
            }
            Expr::Call { callee, args } => self.eval_call(expr, *callee, args),
            Expr::ElementSelect { receiver, select } => {
                let int = self.eval_expr(expr.with_value(*receiver))?.as_int()?;
                if int.has_unknown {
                    return None;
                }
                match select.as_ref()? {
                    Selector::Bit(index) => {
                        let index = self.eval_int_expr(expr.cont_id, *index)?;
                        if !(0..128).contains(&index) {
                            return None;
                        }
                        Some(ConstValue::Int(ConstInt {
                            value: (int.value >> index) & 1,
                            width: 1,
                            signed: false,
                            has_unknown: false,
                        }))
                    }
                    // `[msb:lsb]` and `[msb-:lsb]` take the descending form;
                    // `[lsb:msb]` ascending ranges swap the bounds.
                    Selector::Range(msb, lsb) | Selector::Descending(msb, lsb) => {
                        self.eval_value_slice(expr.cont_id, &int, *msb, *lsb)
                    }
                    Selector::Ascending(lo, hi) => {
                        self.eval_value_slice(expr.cont_id, &int, *hi, *lo)
                    }
                }
            }
            _ => None,
        }
    }

    fn eval_ident(&mut self, expr: OwnerRef<ExprId>, ident: &Ident) -> Option<ConstValue> {
        let def_id = resolve_value_name(self.db, expr, ident)?.unique()?;
        self.eval_def(def_id)
    }

    /// Evaluates a `scope::member` reference whose receiver resolves to an
    /// instantiable scope (package, module, interface, ...).
    fn eval_scoped_field(
        &mut self,
        expr: OwnerRef<ExprId>,
        receiver: ExprId,
        field: &Option<Ident>,
    ) -> Option<ConstValue> {
        let field = field.clone()?;
        let scope = self.resolve_scope_receiver(expr.cont_id, receiver)?;
        let def_id = self.db.scope(scope).lookup(NameContext::Value, &field).unique()?;
        self.eval_def(def_id)
    }

    /// Resolves the receiver of a scoped reference to its scope owner,
    /// preferring the value namespace and falling back to the type namespace
    /// (package names are type-namespace entries).
    fn resolve_scope_receiver(&self, cont: OwnerId, receiver: ExprId) -> Option<OwnerId> {
        let data = cont.data(self.db);
        let Expr::Ident(ident) = data.expr(receiver) else { return None };
        let expr = OwnerRef::new(cont, receiver);
        for context in [NameContext::Value, NameContext::Type] {
            if let Some(def_id) = resolve_name_in(self.db, expr, ident, context).unique()
                && let Some(scope) = descend_scope(self.db, def_id)
            {
                return Some(scope);
            }
        }
        None
    }

    fn eval_def(&mut self, def_id: DefId) -> Option<ConstValue> {
        let db = self.db;
        let decl = def_id.primary_origin(db).as_decl(db);
        if let Some(decl) = decl.as_ref()
            && let Some(value) = self.overrides.get(decl)
        {
            return Some(value.clone());
        }

        match def_id.kind(db) {
            DefKind::Param | DefKind::Specparam | DefKind::Genvar => {
                let decl = decl?;
                let initializer = decl.cont_id.data(db).declarator(decl.value).initializer?;
                if !self.seen.insert(decl) {
                    return None;
                }
                let value = self.eval_expr(OwnerRef::new(decl.cont_id, initializer));
                self.seen.remove(&decl);
                value
            }
            DefKind::EnumMember => self.eval_enum_member(def_id),
            _ => None,
        }
    }

    fn eval_enum_member(&mut self, def_id: DefId) -> Option<ConstValue> {
        let DefOriginLoc::EnumMember { enum_id, index } =
            def_id.primary_origin(self.db).loc(self.db).clone()
        else {
            return None;
        };
        let index = index as usize;
        let data = enum_id.cont_id.data(self.db);
        let enum_def = data.enum_def(enum_id.value);

        // The nearest member at or before `index` with an explicit initializer
        // anchors the implicit value sequence; members without one count up
        // from the anchor (or from zero when no member assigns a value).
        for offset in (0..=index).rev() {
            let Some(initializer) = enum_def.members.get(offset)?.initializer else { continue };
            let mut value = match self.eval_expr(OwnerRef::new(enum_id.cont_id, initializer))? {
                ConstValue::Int(int) if !int.has_unknown => int.value,
                _ => return None,
            };
            value += (index - offset) as i128;
            return Some(ConstValue::Int(ConstInt {
                value,
                width: self.enum_width(enum_id).unwrap_or(32) as u32,
                signed: false,
                has_unknown: false,
            }));
        }
        Some(ConstValue::Int(ConstInt {
            value: index as i128,
            width: self.enum_width(enum_id).unwrap_or(32) as u32,
            signed: false,
            has_unknown: false,
        }))
    }

    fn eval_concat(&mut self, expr: OwnerRef<ExprId>, items: &[ExprId]) -> Option<ConstValue> {
        let mut pattern = 0u128;
        let mut width = 0u32;
        for item in items {
            let int = self.eval_expr(expr.with_value(*item))?.as_int()?;
            if int.has_unknown {
                return None;
            }
            let item_width = int.width.clamp(1, MAX_BITS);
            width = width.checked_add(item_width)?;
            if width > MAX_BITS {
                return None;
            }
            pattern = (pattern << item_width) | pattern_of(&int, item_width);
        }
        Some(ConstValue::Int(int_from_pattern(pattern, width, false)))
    }

    fn eval_multi_concat(
        &mut self,
        expr: OwnerRef<ExprId>,
        rep: ExprId,
        items: &[ExprId],
    ) -> Option<ConstValue> {
        let rep = self.eval_expr(expr.with_value(rep))?.as_int()?;
        if rep.has_unknown || rep.value < 0 {
            return None;
        }
        let inner = self.eval_concat(expr, items)?.as_int()?;
        if inner.has_unknown {
            return None;
        }
        let inner_width = inner.width.max(1);
        let rep_count = u32::try_from(rep.value).ok()?;
        let width = inner_width.checked_mul(rep_count)?;
        if width > MAX_BITS {
            return None;
        }
        let inner_pattern = pattern_of(&inner, inner_width);
        let mut pattern = 0u128;
        for _ in 0..rep_count {
            pattern = (pattern << inner_width) | inner_pattern;
        }
        Some(ConstValue::Int(int_from_pattern(pattern, width, false)))
    }

    fn apply_cast(&mut self, cont: OwnerId, ty: &DataTy, value: ConstValue) -> Option<ConstValue> {
        let width = u32::try_from(self.data_ty_width(cont, ty)?).ok()?;
        let signed = self.data_ty_signed(cont, ty).unwrap_or(false);
        match value {
            ConstValue::Int(int) => {
                if int.has_unknown {
                    return Some(ConstValue::Int(ConstInt {
                        value: 0,
                        width,
                        signed,
                        has_unknown: true,
                    }));
                }
                if width > MAX_BITS {
                    // The mathematical value is already representable; only the
                    // declared width and signing change.
                    return Some(ConstValue::Int(ConstInt { width, signed, ..int }));
                }
                let pattern = (int.value as u128) & mask_for(width);
                Some(ConstValue::Int(int_from_pattern(pattern, width, signed)))
            }
            ConstValue::Real(value) => Some(ConstValue::Int(ConstInt {
                value: value.round() as i128,
                width,
                signed,
                has_unknown: false,
            })),
            ConstValue::Str(_) => None,
        }
    }

    fn eval_call(
        &mut self,
        expr: OwnerRef<ExprId>,
        callee: ExprId,
        args: &[Arg],
    ) -> Option<ConstValue> {
        let data = expr.cont_id.data(self.db);
        let Expr::Ident(name) = data.expr(callee) else { return None };
        match name.as_str() {
            "$clog2" => {
                let value = self.int_arg(expr, args, 0)?;
                Some(ConstValue::Int(ConstInt {
                    value: clog2(value)?,
                    width: 32,
                    signed: false,
                    has_unknown: false,
                }))
            }
            "$ceil" | "$floor" => {
                let value = self.real_arg(expr, args, 0)?;
                let value = if name == "$ceil" { value.ceil() } else { value.floor() };
                Some(ConstValue::Real(value))
            }
            "$sqrt" | "$ln" | "$log10" | "$exp" => {
                let value = self.real_arg(expr, args, 0)?;
                let value = match name.as_str() {
                    "$sqrt" => value.sqrt(),
                    "$ln" => value.ln(),
                    "$log10" => value.log10(),
                    _ => value.exp(),
                };
                value.is_finite().then_some(ConstValue::Real(value))
            }
            "$pow" => {
                let base = self.real_arg(expr, args, 0)?;
                let exponent = self.real_arg(expr, args, 1)?;
                let value = base.powf(exponent);
                value.is_finite().then_some(ConstValue::Real(value))
            }
            "$signed" | "$unsigned" => {
                let mut int = self.eval_arg(expr, args, 0)?.as_int()?;
                int.signed = name == "$signed";
                Some(ConstValue::Int(int))
            }
            "$countones" => {
                let int = self.eval_arg(expr, args, 0)?.as_int()?;
                if int.has_unknown {
                    return None;
                }
                let width = int.width.min(MAX_BITS);
                let ones = pattern_of(&int, width).count_ones();
                Some(ConstValue::Int(ConstInt {
                    value: ones as i128,
                    width: 32,
                    signed: false,
                    has_unknown: false,
                }))
            }
            "$bits" => {
                let bits = self.bits_arg(expr, args)?;
                Some(ConstValue::Int(ConstInt {
                    value: bits as i128,
                    width: 32,
                    signed: false,
                    has_unknown: false,
                }))
            }
            "$size" | "$left" | "$right" | "$high" | "$low" => {
                self.eval_dimension_fn(expr, args, name)
            }
            "$dimensions" | "$unpacked_dimensions" => self.eval_dimension_count(expr, args, name),
            _ => None,
        }
    }

    fn eval_dimension_fn(
        &mut self,
        expr: OwnerRef<ExprId>,
        args: &[Arg],
        name: &str,
    ) -> Option<ConstValue> {
        let arg = self.arg_expr(args, 0)?;
        let dims = self.dims_info(expr.cont_id, arg)?;
        let dim_index = match args.len() {
            1 => 1usize,
            _ => usize::try_from(self.int_arg(expr, args, 1)?).ok()?,
        };
        let (left, right) = match dim_index {
            0 => dims.packed?,
            _ => *dims.unpacked.get(dim_index.checked_sub(1)?)?,
        };
        let value = match name {
            "$left" => left,
            "$right" => right,
            "$high" => left.max(right),
            "$low" => left.min(right),
            _ => (left - right).abs() + 1,
        };
        Some(ConstValue::Int(ConstInt { value, width: 32, signed: false, has_unknown: false }))
    }

    fn eval_dimension_count(
        &mut self,
        expr: OwnerRef<ExprId>,
        args: &[Arg],
        name: &str,
    ) -> Option<ConstValue> {
        let arg = self.arg_expr(args, 0)?;
        let dims = self.dims_info(expr.cont_id, arg)?;
        let count = if name == "$dimensions" {
            dims.packed_count + dims.unpacked_total
        } else {
            dims.unpacked_total
        };
        Some(ConstValue::Int(ConstInt {
            value: count as i128,
            width: 32,
            signed: false,
            has_unknown: false,
        }))
    }

    fn bits_arg(&mut self, expr: OwnerRef<ExprId>, args: &[Arg]) -> Option<u64> {
        let arg = self.arg_expr(args, 0)?;
        let inner = self.unwrap_value_expr(expr.cont_id, arg)?;

        // A string value contributes 8 bits per character.
        if let Some(ConstValue::Str(text)) = self.eval_expr(OwnerRef::new(expr.cont_id, inner)) {
            return Some(8 * text.len() as u64);
        }

        // A `type` or an expression resolving to a declaration or typedef.
        if let Some(bits) = self.bits_via_name(expr.cont_id, inner) {
            return Some(bits);
        }

        // Fall back to the inferred type's packed width.
        let ty = self.db.infer_expr(OwnerRef::new(expr.cont_id, inner));
        self.width_of_ty(ty.ty())
    }

    fn bits_via_name(&mut self, cont: OwnerId, arg: ExprId) -> Option<u64> {
        let data = cont.data(self.db);
        match data.expr(arg) {
            Expr::Ident(ident) => {
                for context in [NameContext::Value, NameContext::Type] {
                    if let Some(def_id) =
                        resolve_name_at(self.db, cont, ident, context, None).unique()
                        && let Some(bits) = self.bits_of_def(def_id)
                    {
                        return Some(bits);
                    }
                }
                None
            }
            Expr::Field { receiver, field } => {
                let field = field.clone()?;
                let scope = self.resolve_scope_receiver(cont, *receiver)?;
                for context in [NameContext::Type, NameContext::Value] {
                    if let Some(def_id) = self.db.scope(scope).lookup(context, &field).unique()
                        && let Some(bits) = self.bits_of_def(def_id)
                    {
                        return Some(bits);
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn bits_of_def(&mut self, def_id: DefId) -> Option<u64> {
        let db = self.db;
        match def_id.kind(db) {
            DefKind::Param
            | DefKind::Variable
            | DefKind::Net
            | DefKind::Port
            | DefKind::Genvar
            | DefKind::Specparam => {
                let decl = def_id.primary_origin(db).as_decl(db)?;
                let data_ty = data_ty_of_decl(db, decl)?;
                // Normalizing with the definition as owner keeps unions typed
                // (`Ty::Union` needs an owner to come out of normalization).
                let ty =
                    normalize_data_ty_with_owner(db, decl.cont_id, data_ty.clone(), Some(def_id))
                        .ty;
                let mut bits = self.width_of_ty(&ty)?;
                // A parameter declared without a type takes the type of its
                // assignment expression (IEEE 1800-2023 6.20.3). The HIR only
                // records the implicit 1-bit logic type, and slang reports
                // minimal widths for unsized literals, so integer-valued
                // untyped parameters are treated as 32-bit `int`.
                if bits == 1
                    && matches!(data_ty, DataTy::Builtin(ref id) if id.get() == &BuiltinDataTy::default())
                    && let Some(value) = self.eval_def(def_id)
                {
                    bits = match value {
                        ConstValue::Int(_) => 32,
                        ConstValue::Real(_) => 64,
                        ConstValue::Str(text) => 8 * text.len() as u64,
                    };
                }
                let data = decl.cont_id.data(db);
                for dim in &data.declarator(decl.value).dimensions {
                    let dim = dim.as_ref()?;
                    bits = bits.checked_mul(self.dimension_width(decl.cont_id, dim)?)?;
                }
                Some(bits)
            }
            DefKind::Typedef => {
                let typedef = def_id.primary_origin(db).as_typedef(db)?;
                let data_ty = typedef.cont_id.data(db).typedef(typedef.value).ty.clone()?;
                let owner = DefId::from_source(db, typedef);
                let ty = normalize_data_ty_with_owner(db, typedef.cont_id, data_ty, Some(owner)).ty;
                // NOTE: array dimensions trailing the typedef name are not
                // modeled by the HIR and therefore not counted here.
                self.width_of_ty(&ty)
            }
            DefKind::StructMember => {
                let DefOriginLoc::StructMember { struct_id, index } =
                    def_id.primary_origin(db).loc(db).clone()
                else {
                    return None;
                };
                let data = struct_id.cont_id.data(db);
                let member = data.struct_def(struct_id.value).members.get(index as usize)?;
                let ty_ref = member.ty.as_ref()?;
                let ty = normalize_data_ty(db, ty_ref.cont_id, ty_ref.value.clone()).ty;
                let mut bits = self.width_of_ty(&ty)?;
                for dim in &member.dimensions {
                    let dim = dim.as_ref()?;
                    bits = bits.checked_mul(self.dimension_width(ty_ref.cont_id, dim)?)?;
                }
                Some(bits)
            }
            DefKind::EnumMember => {
                let DefOriginLoc::EnumMember { enum_id, .. } =
                    def_id.primary_origin(db).loc(db).clone()
                else {
                    return None;
                };
                Some(self.enum_width(enum_id).unwrap_or(32))
            }
            _ => None,
        }
    }

    /// Resolved dimension bounds for a `$size`-family argument: the packed
    /// "dimension 0" plus every constant unpacked dimension.
    fn dims_info(&mut self, cont: OwnerId, arg: ExprId) -> Option<DimsInfo> {
        let inner = self.unwrap_value_expr(cont, arg)?;
        let data = cont.data(self.db);
        let Expr::Ident(ident) = data.expr(inner) else { return None };
        let def_id = resolve_name_at(self.db, cont, ident, NameContext::Value, None).unique()?;
        let decl = def_id.primary_origin(self.db).as_decl(self.db)?;

        let data_ty = data_ty_of_decl(self.db, decl)?;
        let ty = normalize_data_ty(self.db, decl.cont_id, data_ty).ty;
        let (packed, packed_count) = self.packed_dimension_info(&ty)?;
        let mut unpacked = Vec::new();
        let mut unpacked_total = 0u64;
        let decl_data = decl.cont_id.data(self.db);
        for dim in &decl_data.declarator(decl.value).dimensions {
            unpacked_total += 1;
            let Some(dim) = dim.as_ref() else { continue };
            match dim {
                Dimension::Range(left, right) => {
                    let left = self.eval_int_expr(decl.cont_id, *left)?;
                    let right = self.eval_int_expr(decl.cont_id, *right)?;
                    unpacked.push((left, right));
                }
                Dimension::Size(size) => {
                    let size = self.eval_int_expr(decl.cont_id, *size)?;
                    (size >= 0).then(|| unpacked.push((0, size - 1)))?;
                }
                // Queues, associative arrays, and dynamic arrays have
                // data-dependent bounds; they still count as dimensions.
                Dimension::Queue(_)
                | Dimension::Assoc(_)
                | Dimension::Wildcard
                | Dimension::Dynamic => {}
            }
        }
        Some(DimsInfo { packed, packed_count, unpacked, unpacked_total })
    }

    fn packed_dimension_info(&mut self, ty: &Ty) -> Option<(Option<(i128, i128)>, u64)> {
        match ty {
            Ty::Alias { target, .. } => self.packed_dimension_info(target),
            Ty::Builtin(crate::ty::BuiltinTy::Data { id, container }) => match id.get() {
                BuiltinDataTy::Int { kind, .. } => {
                    Some((Some((int_kind_width(*kind) as i128 - 1, 0)), 1))
                }
                BuiltinDataTy::Vector { dimensions, .. } => {
                    let count = dimensions.len().max(1) as u64;
                    let bounds = match dimensions.first().and_then(|dim| *dim) {
                        None => Some((0, 0)),
                        Some(Dimension::Range(left, right)) => {
                            let left = self.eval_int_expr(*container, left)?;
                            let right = self.eval_int_expr(*container, right)?;
                            Some((left, right))
                        }
                        Some(Dimension::Size(size)) => {
                            let size = self.eval_int_expr(*container, size)?;
                            (size >= 0).then_some((0, size - 1))
                        }
                        _ => None,
                    };
                    Some((bounds, count))
                }
                BuiltinDataTy::Real(_) | BuiltinDataTy::String => Some((Some((63, 0)), 1)),
                BuiltinDataTy::Event | BuiltinDataTy::Chandle | BuiltinDataTy::Void => None,
            },
            Ty::Struct(_) | Ty::Union(_) | Ty::Enum(_) => Some((None, 1)),
            _ => None,
        }
    }

    fn enum_width(&mut self, enum_id: OwnerRef<EnumId>) -> Option<u64> {
        let base_ty = enum_id.cont_id.data(self.db).enum_def(enum_id.value).base_ty.clone()?;
        let ty = normalize_data_ty(self.db, enum_id.cont_id, base_ty).ty;
        self.width_of_ty(&ty)
    }

    fn width_of_ty(&mut self, ty: &Ty) -> Option<u64> {
        match ty {
            Ty::Alias { target, .. } => self.width_of_ty(target),
            Ty::Builtin(crate::ty::BuiltinTy::Data { id, container }) => match id.get() {
                BuiltinDataTy::Int { kind, .. } => Some(int_kind_width(*kind) as u64),
                BuiltinDataTy::Vector { dimensions, .. } => {
                    let mut product = 1u64;
                    for dim in dimensions {
                        let dim = dim.as_ref()?;
                        product = product.checked_mul(self.dimension_width(*container, dim)?)?;
                    }
                    Some(product)
                }
                BuiltinDataTy::Real(Real::ShortReal) => Some(32),
                BuiltinDataTy::Real(_) => Some(64),
                BuiltinDataTy::String
                | BuiltinDataTy::Event
                | BuiltinDataTy::Chandle
                | BuiltinDataTy::Void => None,
            },
            Ty::Struct(struct_id) => self.aggregate_width(*struct_id),
            Ty::Union(def_id) => match def_id.data_type(self.db) {
                Some(DataTy::Struct(struct_id)) => self.aggregate_width(struct_id),
                _ => None,
            },
            Ty::Enum(def_id) => {
                let Some(DataTy::Enum(enum_id)) = def_id.data_type(self.db) else {
                    return Some(32);
                };
                Some(self.enum_width(enum_id).unwrap_or(32))
            }
            _ => None,
        }
    }

    fn aggregate_width(&mut self, struct_id: OwnerRef<StructId>) -> Option<u64> {
        let data = struct_id.cont_id.data(self.db);
        let def = data.struct_def(struct_id.value);
        let mut sum = 0u64;
        let mut max = 0u64;
        for member in &def.members {
            let ty_ref = member.ty.as_ref()?;
            let ty = normalize_data_ty(self.db, ty_ref.cont_id, ty_ref.value.clone()).ty;
            let mut bits = self.width_of_ty(&ty)?;
            for dim in &member.dimensions {
                let dim = dim.as_ref()?;
                bits = bits.checked_mul(self.dimension_width(ty_ref.cont_id, dim)?)?;
            }
            sum = sum.checked_add(bits)?;
            max = max.max(bits);
        }
        Some(match def.kind {
            StructKind::Struct => sum,
            StructKind::Union => max,
        })
    }

    fn data_ty_width(&mut self, cont: OwnerId, data_ty: &DataTy) -> Option<u64> {
        let ty = normalize_data_ty(self.db, cont, data_ty.clone()).ty;
        self.width_of_ty(&ty)
    }

    fn data_ty_signed(&mut self, cont: OwnerId, data_ty: &DataTy) -> Option<bool> {
        let ty = normalize_data_ty(self.db, cont, data_ty.clone()).ty;
        Some(match ty {
            Ty::Builtin(crate::ty::BuiltinTy::Data { id, .. }) => match id.get() {
                BuiltinDataTy::Int { signing, .. } | BuiltinDataTy::Vector { signing, .. } => {
                    *signing
                }
                BuiltinDataTy::Real(_) => false,
                _ => return None,
            },
            Ty::Struct(struct_id) => {
                struct_id.cont_id.data(self.db).struct_def(struct_id.value).signing?
            }
            Ty::Enum(_) | Ty::Union(_) => false,
            _ => return None,
        })
    }

    fn dimension_width(&mut self, cont: OwnerId, dim: &Dimension) -> Option<u64> {
        match dim {
            Dimension::Range(left, right) => {
                let left = self.eval_int_expr(cont, *left)?;
                let right = self.eval_int_expr(cont, *right)?;
                u64::try_from(left.checked_sub(right)?.abs().checked_add(1)?).ok()
            }
            Dimension::Size(size) => u64::try_from(self.eval_int_expr(cont, *size)?).ok(),
            Dimension::Queue(_)
            | Dimension::Assoc(_)
            | Dimension::Wildcard
            | Dimension::Dynamic => None,
        }
    }

    fn eval_int_expr(&mut self, cont: OwnerId, expr: ExprId) -> Option<i128> {
        match self.eval_expr(OwnerRef::new(cont, expr))? {
            ConstValue::Int(int) if !int.has_unknown => Some(int.value),
            _ => None,
        }
    }

    /// Bit-slice of an integer value, taken from the mathematical value's
    /// two's-complement representation. Out-of-range or reversed bounds with
    /// negative positions are refused.
    fn eval_value_slice(
        &mut self,
        cont: OwnerId,
        int: &ConstInt,
        msb: ExprId,
        lsb: ExprId,
    ) -> Option<ConstValue> {
        let mut msb = self.eval_int_expr(cont, msb)?;
        let mut lsb = self.eval_int_expr(cont, lsb)?;
        if msb < lsb {
            std::mem::swap(&mut msb, &mut lsb);
        }
        if lsb < 0 || msb >= 128 {
            return None;
        }
        let width = (msb - lsb + 1) as u32;
        let pattern = ((int.value as u128) >> lsb) & mask_for(width);
        Some(ConstValue::Int(int_from_pattern(pattern, width, false)))
    }

    fn arg_expr(&self, args: &[Arg], index: usize) -> Option<ExprId> {
        match args.get(index)? {
            Arg::Ordered(expr) | Arg::Named { expr, .. } => Some(*expr),
            Arg::Empty => None,
        }
    }

    /// Unwraps the `Property(Simple(Sequence(Simple(expr))))` envelope that
    /// lowering places around subroutine-call arguments.
    fn unwrap_value_expr(&self, cont: OwnerId, expr: ExprId) -> Option<ExprId> {
        let data = cont.data(self.db);
        match data.expr(expr) {
            Expr::Property(PropertyExpr::Simple(sequence)) => {
                let sequence_data = cont.data(self.db);
                match sequence_data.expr(*sequence) {
                    Expr::Sequence(SequenceExpr::Simple { expr, repetition: None }) => Some(*expr),
                    _ => None,
                }
            }
            _ => Some(expr),
        }
    }

    fn eval_arg(
        &mut self,
        expr: OwnerRef<ExprId>,
        args: &[Arg],
        index: usize,
    ) -> Option<ConstValue> {
        let arg = self.arg_expr(args, index)?;
        let inner = self.unwrap_value_expr(expr.cont_id, arg)?;
        self.eval_expr(OwnerRef::new(expr.cont_id, inner))
    }

    fn int_arg(&mut self, expr: OwnerRef<ExprId>, args: &[Arg], index: usize) -> Option<i128> {
        let int = self.eval_arg(expr, args, index)?.as_int()?;
        (!int.has_unknown).then_some(int.value)
    }

    fn real_arg(&mut self, expr: OwnerRef<ExprId>, args: &[Arg], index: usize) -> Option<f64> {
        self.eval_arg(expr, args, index)?.promote_real()
    }
}

struct DimsInfo {
    packed: Option<(i128, i128)>,
    packed_count: u64,
    unpacked: Vec<(i128, i128)>,
    unpacked_total: u64,
}

/// Resolves `ident` as a value reference at the expression's own source
/// position, mirroring the type inference path.
fn resolve_value_name(
    db: &dyn TyDb,
    expr: OwnerRef<ExprId>,
    ident: &Ident,
) -> Option<Resolution<DefId>> {
    Some(resolve_name_in(db, expr, ident, NameContext::Value))
}

fn resolve_name_in(
    db: &dyn TyDb,
    expr: OwnerRef<ExprId>,
    ident: &Ident,
    context: NameContext,
) -> Resolution<DefId> {
    let file_id = expr.cont_id.file(db);
    let reference = db
        .body_with_source_map(expr.cont_id)
        .source_map()
        .expr_srcs
        .hir_to_src(expr.value)
        .map(|source| NameRef { position: InFile::new(file_id, source), kind: RefKind::Value });
    resolve_name_at(db, expr.cont_id, ident, context, reference.as_ref())
}

fn eval_literal(literal: &Literal) -> Option<ConstValue> {
    match literal {
        Literal::Int(int) => Some(ConstValue::Int(const_int_from_svint(int)?)),
        Literal::Float(float) => Some(ConstValue::Real(f64::from(*float))),
        Literal::Str(text) => Some(ConstValue::Str(text.clone())),
        Literal::UnbasedUnsized(bit) => Some(ConstValue::Int(ConstInt {
            value: matches!(bit, Bit::H) as i128,
            width: 1,
            signed: false,
            has_unknown: matches!(bit, Bit::X | Bit::Z),
        })),
        Literal::Time { .. } | Literal::Null | Literal::Unbounded => None,
    }
}

fn const_int_from_svint(int: &SVInt) -> Option<ConstInt> {
    let width = int.get_bit_width() as u32;
    let has_unknown = int.has_unknown();
    let value = if has_unknown {
        Some(0)
    } else if let Some(word) = int.get_single_word() {
        Some(word_to_i128(word, width, int.is_signed()))
    } else {
        // Values wider than 64 bits only reach Rust as serialized text.
        int.serialize(10).parse::<i128>().ok()
    }?;
    Some(ConstInt { value, width, signed: int.is_signed(), has_unknown })
}

fn word_to_i128(word: u64, width: u32, signed: bool) -> i128 {
    let width = width.min(64);
    let masked = if width == 0 {
        0
    } else if width >= 64 {
        word
    } else {
        word & ((1u64 << width) - 1)
    };
    if signed && width > 0 && (masked >> (width - 1)) & 1 == 1 {
        (masked as i128) - (1i128 << width)
    } else {
        masked as i128
    }
}

fn mask_for(width: u32) -> u128 {
    if width == 0 {
        0
    } else if width >= 128 {
        u128::MAX
    } else {
        (1u128 << width) - 1
    }
}

/// Two's-complement bit pattern of `int` within `width` bits. `i128 as u128`
/// reinterprets the mathematical value modulo 2^128, so masking to `width`
/// yields the correct pattern for negative values as well.
fn pattern_of(int: &ConstInt, width: u32) -> u128 {
    (int.value as u128) & mask_for(width)
}

fn int_from_pattern(pattern: u128, width: u32, signed: bool) -> ConstInt {
    let value = if signed && width > 0 && (pattern >> (width - 1)) & 1 == 1 {
        (pattern as i128) - (1i128 << width)
    } else {
        pattern as i128
    };
    ConstInt { value, width, signed, has_unknown: false }
}

fn clog2(value: i128) -> Option<i128> {
    if value < 0 {
        return None;
    }
    if value <= 1 {
        return Some(0);
    }
    Some((value - 1).checked_ilog2()? as i128 + 1)
}

fn apply_binary(op: BinaryOp, left: &ConstValue, right: &ConstValue) -> Option<ConstValue> {
    match op {
        BinaryOp::Assign(_) => None,
        BinaryOp::Add
        | BinaryOp::Sub
        | BinaryOp::Mul
        | BinaryOp::Div
        | BinaryOp::Mod
        | BinaryOp::Pow => apply_arithmetic(op, left, right),
        BinaryOp::Eq
        | BinaryOp::Neq
        | BinaryOp::CaseEq
        | BinaryOp::CaseNeq
        | BinaryOp::WildEq
        | BinaryOp::WildNeq
        | BinaryOp::Gt
        | BinaryOp::Ge
        | BinaryOp::Lt
        | BinaryOp::Le => apply_comparison(op, left, right),
        BinaryOp::LogAnd | BinaryOp::LogOr => {
            let left = left.truthy()?;
            let right = right.truthy()?;
            let value = if matches!(op, BinaryOp::LogAnd) { left && right } else { left || right };
            Some(ConstValue::Int(ConstInt {
                value: value as i128,
                width: 1,
                signed: false,
                has_unknown: false,
            }))
        }
        BinaryOp::ShiftLeft
        | BinaryOp::ShiftRight
        | BinaryOp::ArithShiftLeft
        | BinaryOp::ArithShiftRight
        | BinaryOp::BitAnd
        | BinaryOp::BitOr
        | BinaryOp::BitXor
        | BinaryOp::BitXnor => apply_bitwise(op, left, right),
    }
}

fn is_real_operand(value: &ConstValue) -> bool {
    matches!(value, ConstValue::Real(_))
}

fn apply_arithmetic(op: BinaryOp, left: &ConstValue, right: &ConstValue) -> Option<ConstValue> {
    // Reals take over as soon as either operand is real.
    if is_real_operand(left) || is_real_operand(right) {
        let left = left.promote_real()?;
        let right = right.promote_real()?;
        return match op {
            BinaryOp::Add => Some(ConstValue::Real(left + right)),
            BinaryOp::Sub => Some(ConstValue::Real(left - right)),
            BinaryOp::Mul => Some(ConstValue::Real(left * right)),
            BinaryOp::Div => Some(ConstValue::Real(left / right)),
            BinaryOp::Pow => {
                let value = left.powf(right);
                value.is_finite().then_some(ConstValue::Real(value))
            }
            _ => None,
        };
    }

    let left = left.as_int()?;
    let right = right.as_int()?;
    if left.has_unknown || right.has_unknown {
        return None;
    }

    let (value, width, signed) = match op {
        BinaryOp::Add => (
            left.value.checked_add(right.value)?,
            int_width(&left, &right),
            int_signed(&left, &right),
        ),
        BinaryOp::Sub => (
            left.value.checked_sub(right.value)?,
            int_width(&left, &right),
            int_signed(&left, &right),
        ),
        BinaryOp::Mul => (
            left.value.checked_mul(right.value)?,
            int_width(&left, &right),
            int_signed(&left, &right),
        ),
        BinaryOp::Div => {
            if right.value == 0 || (left.value == i128::MIN && right.value == -1) {
                return None;
            }
            (
                left.value.checked_div(right.value)?,
                int_width(&left, &right),
                int_signed(&left, &right),
            )
        }
        BinaryOp::Mod => {
            if right.value == 0 || (left.value == i128::MIN && right.value == -1) {
                return None;
            }
            (
                left.value.checked_rem(right.value)?,
                int_width(&left, &right),
                int_signed(&left, &right),
            )
        }
        BinaryOp::Pow => {
            if right.value < 0 {
                // A negative integer exponent produces a real result.
                let value = (left.value as f64).powf(right.value as f64);
                return value.is_finite().then_some(ConstValue::Real(value));
            }
            // Cap the exponent so `checked_pow` cannot spin on huge values.
            if right.value > 128 {
                return None;
            }
            (
                left.value.checked_pow(right.value as u32)?,
                int_width(&left, &right),
                int_signed(&left, &right),
            )
        }
        _ => return None,
    };
    Some(ConstValue::Int(ConstInt { value, width, signed, has_unknown: false }))
}

fn apply_comparison(op: BinaryOp, left: &ConstValue, right: &ConstValue) -> Option<ConstValue> {
    let result = if is_real_operand(left) || is_real_operand(right) {
        let left = left.promote_real()?;
        let right = right.promote_real()?;
        match op {
            BinaryOp::Eq | BinaryOp::CaseEq | BinaryOp::WildEq => left == right,
            BinaryOp::Neq | BinaryOp::CaseNeq | BinaryOp::WildNeq => left != right,
            BinaryOp::Gt => left > right,
            BinaryOp::Ge => left >= right,
            BinaryOp::Lt => left < right,
            BinaryOp::Le => left <= right,
            _ => return None,
        }
    } else if let (Some(left), Some(right)) = (as_string(left), as_string(right)) {
        match op {
            BinaryOp::Eq | BinaryOp::CaseEq | BinaryOp::WildEq => left == right,
            BinaryOp::Neq | BinaryOp::CaseNeq | BinaryOp::WildNeq => left != right,
            BinaryOp::Gt => left > right,
            BinaryOp::Ge => left >= right,
            BinaryOp::Lt => left < right,
            BinaryOp::Le => left <= right,
            _ => return None,
        }
    } else {
        let left = left.as_int()?;
        let right = right.as_int()?;
        if left.has_unknown || right.has_unknown {
            return None;
        }
        let equal = left.value == right.value;
        match op {
            BinaryOp::Eq | BinaryOp::CaseEq | BinaryOp::WildEq => equal,
            BinaryOp::Neq | BinaryOp::CaseNeq | BinaryOp::WildNeq => !equal,
            BinaryOp::Gt => left.value > right.value,
            BinaryOp::Ge => left.value >= right.value,
            BinaryOp::Lt => left.value < right.value,
            BinaryOp::Le => left.value <= right.value,
            _ => return None,
        }
    };
    Some(ConstValue::Int(ConstInt {
        value: result as i128,
        width: 1,
        signed: false,
        has_unknown: false,
    }))
}

fn as_string(value: &ConstValue) -> Option<&str> {
    match value {
        ConstValue::Str(text) => Some(text),
        _ => None,
    }
}

fn apply_bitwise(op: BinaryOp, left: &ConstValue, right: &ConstValue) -> Option<ConstValue> {
    let left = left.as_int()?;
    let right = right.as_int()?;
    if left.has_unknown || right.has_unknown {
        return None;
    }

    match op {
        BinaryOp::ShiftLeft | BinaryOp::ArithShiftLeft => {
            let amount = u32::try_from(right.value).ok()?;
            let value = left.value.checked_shl(amount)?;
            Some(ConstValue::Int(ConstInt {
                value,
                width: left.width,
                signed: left.signed,
                has_unknown: false,
            }))
        }
        BinaryOp::ShiftRight => {
            let amount = u32::try_from(right.value).ok()?;
            let value = if amount >= 128 { 0 } else { ((left.value as u128) >> amount) as i128 };
            Some(ConstValue::Int(ConstInt {
                value,
                width: left.width,
                signed: false,
                has_unknown: false,
            }))
        }
        BinaryOp::ArithShiftRight => {
            let amount = u32::try_from(right.value).ok()?;
            let value = if amount >= 128 {
                if left.value < 0 { -1 } else { 0 }
            } else {
                left.value.checked_shr(amount)?
            };
            Some(ConstValue::Int(ConstInt {
                value,
                width: left.width,
                signed: left.signed,
                has_unknown: false,
            }))
        }
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::BitXnor => {
            if left.width > MAX_BITS || right.width > MAX_BITS {
                return None;
            }
            let width = left.width.max(right.width).max(1);
            let left_pattern = pattern_of(&left, width);
            let right_pattern = pattern_of(&right, width);
            let pattern = match op {
                BinaryOp::BitAnd => left_pattern & right_pattern,
                BinaryOp::BitOr => left_pattern | right_pattern,
                BinaryOp::BitXor => left_pattern ^ right_pattern,
                _ => left_pattern ^ right_pattern ^ mask_for(width),
            };
            Some(ConstValue::Int(int_from_pattern(pattern, width, left.signed && right.signed)))
        }
        _ => None,
    }
}

fn apply_unary(op: UnaryOp, value: &ConstValue) -> Option<ConstValue> {
    match (op, value) {
        (UnaryOp::Pos, ConstValue::Int(int)) => Some(ConstValue::Int(*int)),
        (UnaryOp::Pos, ConstValue::Real(real)) => Some(ConstValue::Real(*real)),
        (UnaryOp::Neg, ConstValue::Int(int)) if !int.has_unknown => {
            Some(ConstValue::Int(ConstInt { value: int.value.checked_neg()?, ..*int }))
        }
        (UnaryOp::Neg, ConstValue::Real(real)) => Some(ConstValue::Real(-*real)),
        (UnaryOp::LogNeg, value) => {
            let truthy = value.truthy()?;
            Some(ConstValue::Int(ConstInt {
                value: (!truthy) as i128,
                width: 1,
                signed: false,
                has_unknown: false,
            }))
        }
        (_, ConstValue::Int(int)) if !int.has_unknown => {
            let width = int.width.clamp(1, MAX_BITS);
            if int.width > MAX_BITS {
                return None;
            }
            let pattern = pattern_of(int, width);
            match op {
                UnaryOp::BitNeg => Some(ConstValue::Int(int_from_pattern(
                    !pattern & mask_for(width),
                    width,
                    int.signed,
                ))),
                UnaryOp::ReducAnd => Some(reduction((pattern == mask_for(width)) as i128)),
                UnaryOp::ReducNand => Some(reduction(((pattern == mask_for(width)) as i128) ^ 1)),
                UnaryOp::ReducOr => Some(reduction((pattern != 0) as i128)),
                UnaryOp::ReducNor => Some(reduction((pattern == 0) as i128)),
                UnaryOp::ReducXor => Some(reduction((pattern.count_ones() & 1) as i128)),
                UnaryOp::ReducXnor => Some(reduction(((pattern.count_ones() & 1) as i128) ^ 1)),
                _ => None,
            }
        }
        _ => None,
    }
}

fn reduction(value: i128) -> ConstValue {
    ConstValue::Int(ConstInt { value, width: 1, signed: false, has_unknown: false })
}

fn int_width(left: &ConstInt, right: &ConstInt) -> u32 {
    left.width.max(right.width)
}

fn int_signed(left: &ConstInt, right: &ConstInt) -> bool {
    left.signed && right.signed
}

fn int_kind_width(kind: IntKind) -> usize {
    match kind {
        IntKind::Byte => 8,
        IntKind::ShortInt => 16,
        IntKind::Int => 32,
        IntKind::LongInt => 64,
        IntKind::Integer => 32,
        IntKind::Time => 64,
    }
}
