use std::fmt::{self, Debug};

use hir_def::{
    aggregate::StructKind,
    constraint::DistItem,
    container::OwnerRef,
    expr::{
        Arg, AssignOp, AssignmentPattern, AssignmentPatternItem, BinaryOp, Expr, ExprId, IncDecOp,
        InsideRange, PropertyCaseItem, PropertyExpr, Selector, SequenceExpr, SequenceRepetition,
        StreamOp, UnaryOp,
        data_ty::{
            BuiltinDataTy, DataTy, Dimension, IntKind, Real, TypePathKind, TypeRef, VecKind,
        },
        declarator::DeclId,
    },
    literal::Literal,
    module::port::{PortDirection, PortHeader},
    subroutine::SubroutinePortDir,
    ty::{NetKind, NetType},
    typedef::TypedefId,
};
use syntax::value::TimeUnit;
use triomphe::Arc;

use hir_def::db::HirDefDb;

pub struct HirFormatter<'a> {
    pub db: &'a dyn HirDefDb,
    f: &'a mut dyn HirWrite,
    simplified_ty: bool,
}

pub trait HirWrite: fmt::Write {}

impl HirWrite for String {}

impl HirWrite for fmt::Formatter<'_> {}

impl HirFormatter<'_> {
    pub fn write_str(&mut self, s: &str) -> Result<(), HirDisplayError> {
        self.f.write_str(s)?;
        Ok(())
    }

    pub fn write_char(&mut self, c: char) -> Result<(), HirDisplayError> {
        self.write_str(c.encode_utf8(&mut [0; 4]))
    }
}
fn fmt_type_ref(ty: &TypeRef, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
    if ty.recovery().is_some() {
        return f.write_str("<invalid type path>");
    }
    let separator = match ty.path_kind() {
        TypePathKind::Unqualified => "",
        TypePathKind::Package => "::",
        TypePathKind::Hierarchical => ".",
    };
    for (index, segment) in ty.segments().iter().enumerate() {
        if index != 0 {
            f.write_str(separator)?;
        }
        f.write_str(segment)?;
    }
    Ok(())
}

#[derive(Debug)]
pub struct HirDisplayError(fmt::Error);

impl From<fmt::Error> for HirDisplayError {
    fn from(err: fmt::Error) -> Self {
        HirDisplayError(err)
    }
}

pub trait HirDisplay {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError>;

    fn display_source(&self, db: &dyn HirDefDb) -> Result<String, HirDisplayError> {
        let mut res = String::new();
        self.hir_fmt(&mut HirFormatter { db, f: &mut res, simplified_ty: false })?;
        Ok(res)
    }

    fn display_signature(&self, db: &dyn HirDefDb) -> Result<String, HirDisplayError> {
        let mut res = String::new();
        self.hir_fmt(&mut HirFormatter { db, f: &mut res, simplified_ty: true })?;
        Ok(res)
    }
}

impl<T: HirDisplay> HirDisplay for Arc<T> {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        (**self).hir_fmt(f)
    }
}

impl HirDisplay for PortDirection {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        match self {
            PortDirection::Input => f.write_str("input"),
            PortDirection::Output => f.write_str("output"),
            PortDirection::Ref => f.write_str("ref"),
            PortDirection::Inout => f.write_str("inout"),
        }
    }
}

impl HirDisplay for SubroutinePortDir {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        match self {
            SubroutinePortDir::Input => f.write_str("input"),
            SubroutinePortDir::Output => f.write_str("output"),
            SubroutinePortDir::Inout => f.write_str("inout"),
            SubroutinePortDir::Ref => f.write_str("ref"),
            SubroutinePortDir::ConstRef => f.write_str("const ref"),
            SubroutinePortDir::Unknown => Ok(()),
        }
    }
}

impl HirDisplay for OwnerRef<DataTy> {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        match &self.value {
            DataTy::Builtin(ty_id) => match ty_id.get() {
                BuiltinDataTy::Int { kind, signing } => {
                    match kind {
                        IntKind::Byte => f.write_str("byte"),
                        IntKind::ShortInt => f.write_str("shortint"),
                        IntKind::Int => f.write_str("int"),
                        IntKind::LongInt => f.write_str("longint"),
                        IntKind::Integer => f.write_str("integer"),
                        IntKind::Time => f.write_str("time"),
                    }?;
                    if *signing {
                        f.write_str(" signed")?;
                    }
                    Ok(())
                }
                BuiltinDataTy::Vector { kind, signing, dimensions } => {
                    let mut wrote_head = false;
                    match kind {
                        VecKind::Bit => {
                            if !f.simplified_ty {
                                f.write_str("bit")?;
                                wrote_head = true;
                            }
                        }
                        VecKind::Logic => {
                            if !f.simplified_ty {
                                f.write_str("logic")?;
                                wrote_head = true;
                            }
                        }
                        VecKind::Reg => {
                            f.write_str("reg")?;
                            wrote_head = true;
                        }
                    }
                    if *signing {
                        if wrote_head {
                            f.write_str(" ")?;
                        }
                        f.write_str("signed")?;
                        wrote_head = true;
                    }
                    for dim in dimensions.iter().flatten() {
                        if wrote_head {
                            f.write_str(" ")?;
                        }
                        OwnerRef::new(self.cont_id, *dim).hir_fmt(f)?;
                        wrote_head = true;
                    }
                    Ok(())
                }
                BuiltinDataTy::Real(real) => match real {
                    Real::Real => f.write_str("real"),
                    Real::ShortReal => f.write_str("shortreal"),
                    Real::RealTime => f.write_str("realtime"),
                },
                BuiltinDataTy::String => f.write_str("string"),
                BuiltinDataTy::Event => f.write_str("event"),
                BuiltinDataTy::Chandle => f.write_str("chandle"),
                BuiltinDataTy::Void => f.write_str("void"),
            },
            DataTy::Struct(struct_ref) => {
                let cont = struct_ref.cont_id.data(f.db);
                let def = cont.struct_def(struct_ref.value);
                let keyword = match def.kind {
                    StructKind::Struct => "struct",
                    StructKind::Union => "union",
                };
                f.write_str(keyword)?;
                if let Some(name) = &def.name {
                    f.write_str(" ")?;
                    f.write_str(name.as_str())?;
                }
                Ok(())
            }
            DataTy::Named(named) => fmt_type_ref(named, f),
            DataTy::Enum(_) => f.write_str("enum"),
            DataTy::Unsupported(kind) => {
                write!(f.f, "<unsupported {kind:?}>")?;
                Ok(())
            }
        }
    }
}

impl HirDisplay for OwnerRef<PortHeader> {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        let OwnerRef { cont_id, value: port_header } = self;
        match port_header {
            PortHeader::Var { dir, var_kw, ty } => {
                match dir {
                    PortDirection::Input => f.write_str("input ")?,
                    PortDirection::Output => f.write_str("output ")?,
                    PortDirection::Inout => f.write_str("inout ")?,
                    PortDirection::Ref => f.write_str("ref ")?,
                }
                if *var_kw {
                    f.write_str("var ")?;
                }
                OwnerRef::new(*cont_id, ty.clone()).hir_fmt(f)
            }
            PortHeader::Net { dir, net_ty: NetType { kind, ty } } => {
                match dir {
                    PortDirection::Input => f.write_str("input ")?,
                    PortDirection::Output => f.write_str("output ")?,
                    PortDirection::Inout => f.write_str("inout ")?,
                    PortDirection::Ref => f.write_str("ref ")?,
                }
                match *kind {
                    NetKind::Supply0 => f.write_str("supply0 ")?,
                    NetKind::Supply1 => f.write_str("supply1 ")?,
                    NetKind::Tri => f.write_str("tri ")?,
                    NetKind::Triand => f.write_str("triand ")?,
                    NetKind::Trior => f.write_str("trior ")?,
                    NetKind::Tri0 => f.write_str("tri0 ")?,
                    NetKind::Tri1 => f.write_str("tri1 ")?,
                    NetKind::Uwire => f.write_str("uwire ")?,
                    NetKind::Wire => {
                        if !f.simplified_ty {
                            f.write_str("wire ")?
                        }
                    }
                    NetKind::Wand => f.write_str("wand ")?,
                    NetKind::Wor => f.write_str("wor ")?,
                }
                OwnerRef::new(*cont_id, ty.clone()).hir_fmt(f)
            }
            PortHeader::Interface { dir } => {
                match dir {
                    PortDirection::Input => f.write_str("input ")?,
                    PortDirection::Output => f.write_str("output ")?,
                    PortDirection::Inout => f.write_str("inout ")?,
                    PortDirection::Ref => f.write_str("ref ")?,
                }
                f.write_str("interface")
            }
        }
    }
}

impl HirDisplay for OwnerRef<ExprId> {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        let OwnerRef { cont_id, value: expr_id } = self;
        let container = cont_id.data(f.db);
        let expr = container.expr(*expr_id);
        self.with_value(expr).hir_fmt(f)
    }
}

impl HirDisplay for BinaryOp {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        match self {
            BinaryOp::Add => f.write_str("+"),
            BinaryOp::Sub => f.write_str("-"),
            BinaryOp::Mul => f.write_str("*"),
            BinaryOp::Div => f.write_str("/"),
            BinaryOp::Mod => f.write_str("%"),
            BinaryOp::Pow => f.write_str("**"),
            BinaryOp::Eq => f.write_str("=="),
            BinaryOp::Neq => f.write_str("!="),
            BinaryOp::CaseEq => f.write_str("==="),
            BinaryOp::CaseNeq => f.write_str("!=="),
            BinaryOp::WildEq => f.write_str("==?"),
            BinaryOp::WildNeq => f.write_str("!=?"),
            BinaryOp::Gt => f.write_str(">"),
            BinaryOp::Ge => f.write_str(">="),
            BinaryOp::Lt => f.write_str("<"),
            BinaryOp::Le => f.write_str("<="),
            BinaryOp::LogAnd => f.write_str("&&"),
            BinaryOp::LogOr => f.write_str("||"),
            BinaryOp::ShiftRight => f.write_str(">>"),
            BinaryOp::ShiftLeft => f.write_str("<<"),
            BinaryOp::ArithShiftRight => f.write_str(">>>"),
            BinaryOp::ArithShiftLeft => f.write_str("<<<"),
            BinaryOp::BitAnd => f.write_str("&"),
            BinaryOp::BitOr => f.write_str("|"),
            BinaryOp::BitXor => f.write_str("^"),
            BinaryOp::BitXnor => f.write_str("~^"),
            BinaryOp::Assign(op) => match op {
                AssignOp::Assign => f.write_str("="),
                AssignOp::NonBlockAssign => f.write_str("<="),
                AssignOp::AddAssign => f.write_str("+="),
                AssignOp::SubAssign => f.write_str("-="),
                AssignOp::MulAssign => f.write_str("*="),
                AssignOp::DivAssign => f.write_str("/="),
                AssignOp::ModAssign => f.write_str("%="),
                AssignOp::BitAndAssign => f.write_str("&="),
                AssignOp::BitOrAssign => f.write_str("|="),
                AssignOp::BitXorAssign => f.write_str("^="),
                AssignOp::ShiftLeftAssign => f.write_str("<<="),
                AssignOp::ShiftRightAssign => f.write_str(">>="),
                AssignOp::ArithShiftLeftAssign => f.write_str("<<<="),
                AssignOp::ArithShiftRightAssign => f.write_str(">>>="),
            },
        }
    }
}

impl HirDisplay for UnaryOp {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        match self {
            UnaryOp::Pos => f.write_str("+"),
            UnaryOp::Neg => f.write_str("-"),
            UnaryOp::LogNeg => f.write_str("!"),
            UnaryOp::BitNeg => f.write_str("~"),
            UnaryOp::ReducAnd => f.write_str("&"),
            UnaryOp::ReducNand => f.write_str("~&"),
            UnaryOp::ReducOr => f.write_str("|"),
            UnaryOp::ReducNor => f.write_str("~|"),
            UnaryOp::ReducXor => f.write_str("^"),
            UnaryOp::ReducXnor => f.write_str("~^"),
        }
    }
}

fn fmt_selector(
    owner: &OwnerRef<&Expr>,
    f: &mut HirFormatter<'_>,
    selector: Selector,
) -> Result<(), HirDisplayError> {
    match selector {
        Selector::Bit(expr) => {
            f.write_str("[")?;
            owner.with_value(expr).hir_fmt(f)?;
            f.write_str("]")
        }
        Selector::Range(left, right) => {
            f.write_str("[")?;
            owner.with_value(left).hir_fmt(f)?;
            f.write_str(":")?;
            owner.with_value(right).hir_fmt(f)?;
            f.write_str("]")
        }
        Selector::Ascending(left, right) => {
            f.write_str("[")?;
            owner.with_value(left).hir_fmt(f)?;
            f.write_str("+:")?;
            owner.with_value(right).hir_fmt(f)?;
            f.write_str("]")
        }
        Selector::Descending(left, right) => {
            f.write_str("[")?;
            owner.with_value(left).hir_fmt(f)?;
            f.write_str("-:")?;
            owner.with_value(right).hir_fmt(f)?;
            f.write_str("]")
        }
    }
}

fn fmt_repetition(
    owner: &OwnerRef<&Expr>,
    f: &mut HirFormatter<'_>,
    repetition: &SequenceRepetition,
) -> Result<(), HirDisplayError> {
    write!(f.f, " {:?}", repetition.op)?;
    if let Some(selector) = repetition.selector {
        fmt_selector(owner, f, selector)?;
    }
    Ok(())
}

fn fmt_sequence_expr(
    owner: &OwnerRef<&Expr>,
    f: &mut HirFormatter<'_>,
    sequence: &SequenceExpr,
) -> Result<(), HirDisplayError> {
    match sequence {
        SequenceExpr::Simple { expr, repetition } => {
            owner.with_value(*expr).hir_fmt(f)?;
            if let Some(repetition) = repetition {
                fmt_repetition(owner, f, repetition)?;
            }
            Ok(())
        }
        SequenceExpr::Binary { left, op, right } => {
            owner.with_value(*left).hir_fmt(f)?;
            write!(f.f, " {op:?} ")?;
            owner.with_value(*right).hir_fmt(f)
        }
        SequenceExpr::Delayed { first, elements } => {
            if let Some(first) = first {
                owner.with_value(*first).hir_fmt(f)?;
            }
            for element in elements.iter() {
                f.write_str(" ##")?;
                if let Some(delay) = element.delay {
                    f.write_str(" ")?;
                    owner.with_value(delay).hir_fmt(f)?;
                }
                if let Some(range) = element.range {
                    fmt_selector(owner, f, range)?;
                }
                f.write_str(" ")?;
                owner.with_value(element.expr).hir_fmt(f)?;
            }
            Ok(())
        }
        SequenceExpr::Event(_) => f.write_str("<event>"),
        SequenceExpr::Clocking { expr, .. } => {
            f.write_str("<clocking> ")?;
            owner.with_value(*expr).hir_fmt(f)
        }
        SequenceExpr::FirstMatch { expr } => {
            f.write_str("first_match(")?;
            owner.with_value(*expr).hir_fmt(f)?;
            f.write_str(")")
        }
        SequenceExpr::Parenthesized { expr, matches, repetition } => {
            f.write_str("(")?;
            owner.with_value(*expr).hir_fmt(f)?;
            for matched in matches.iter() {
                f.write_str(", ")?;
                owner.with_value(*matched).hir_fmt(f)?;
            }
            f.write_str(")")?;
            if let Some(repetition) = repetition {
                fmt_repetition(owner, f, repetition)?;
            }
            Ok(())
        }
    }
}

fn fmt_property_expr(
    owner: &OwnerRef<&Expr>,
    f: &mut HirFormatter<'_>,
    property: &PropertyExpr,
) -> Result<(), HirDisplayError> {
    match property {
        PropertyExpr::Parenthesized { expr, matches } => {
            f.write_str("(")?;
            owner.with_value(*expr).hir_fmt(f)?;
            for matched in matches.iter() {
                f.write_str(", ")?;
                owner.with_value(*matched).hir_fmt(f)?;
            }
            f.write_str(")")
        }
        PropertyExpr::Simple(expr) => owner.with_value(*expr).hir_fmt(f),
        PropertyExpr::Binary { left, op, right } => {
            owner.with_value(*left).hir_fmt(f)?;
            write!(f.f, " {op:?} ")?;
            owner.with_value(*right).hir_fmt(f)
        }
        PropertyExpr::Conditional { condition, expr, else_expr } => {
            owner.with_value(*condition).hir_fmt(f)?;
            f.write_str(" ? ")?;
            owner.with_value(*expr).hir_fmt(f)?;
            if let Some(else_expr) = else_expr {
                f.write_str(" : ")?;
                owner.with_value(*else_expr).hir_fmt(f)?;
            }
            Ok(())
        }
        PropertyExpr::Unary { op, expr } => {
            write!(f.f, "{op:?} ")?;
            owner.with_value(*expr).hir_fmt(f)
        }
        PropertyExpr::UnarySelect { op, selector, expr } => {
            write!(f.f, "{op:?} ")?;
            if let Some(selector) = selector {
                fmt_selector(owner, f, *selector)?;
            }
            owner.with_value(*expr).hir_fmt(f)
        }
        PropertyExpr::Clocking { expr, .. } => {
            f.write_str("<clocking>")?;
            if let Some(expr) = expr {
                f.write_str(" ")?;
                owner.with_value(*expr).hir_fmt(f)?;
            }
            Ok(())
        }
        PropertyExpr::StrongWeak { strong, expr } => {
            f.write_str(if *strong { "strong(" } else { "weak(" })?;
            owner.with_value(*expr).hir_fmt(f)?;
            f.write_str(")")
        }
        PropertyExpr::AcceptOn { condition, expr } => {
            f.write_str("accept_on(")?;
            owner.with_value(*condition).hir_fmt(f)?;
            f.write_str(") ")?;
            owner.with_value(*expr).hir_fmt(f)
        }
        PropertyExpr::Case { expr, items } => {
            f.write_str("case (")?;
            owner.with_value(*expr).hir_fmt(f)?;
            f.write_str(") ")?;
            for item in items {
                match item {
                    PropertyCaseItem::Default { expr } => {
                        f.write_str("default: ")?;
                        owner.with_value(*expr).hir_fmt(f)?;
                    }
                    PropertyCaseItem::Standard { expressions, expr } => {
                        for (index, expression) in expressions.iter().enumerate() {
                            if index != 0 {
                                f.write_str(", ")?;
                            }
                            owner.with_value(*expression).hir_fmt(f)?;
                        }
                        f.write_str(": ")?;
                        owner.with_value(*expr).hir_fmt(f)?;
                    }
                }
                f.write_str("; ")?;
            }
            f.write_str("endcase")
        }
    }
}

impl HirDisplay for OwnerRef<&Expr> {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        match self.value {
            Expr::Missing => f.write_str("<missing>"),
            Expr::Error(kind) => {
                write!(f.f, "<error {kind:?}>")?;
                Ok(())
            }
            Expr::Unsupported(kind) => {
                write!(f.f, "<unsupported {kind:?}>")?;
                Ok(())
            }
            Expr::AssignmentPattern { ty, pattern } => {
                if let Some(ty) = ty {
                    self.with_value(ty.clone()).hir_fmt(f)?;
                }
                f.write_str("'")?;
                match pattern {
                    AssignmentPattern::Simple(items) => {
                        f.write_str("{")?;
                        let mut first = true;
                        for expr in items.iter() {
                            if !first {
                                f.write_str(", ")?;
                            }
                            self.with_value(*expr).hir_fmt(f)?;
                            first = false;
                        }
                        f.write_str("}")
                    }
                    AssignmentPattern::Structured(items) => {
                        f.write_str("{")?;
                        let mut first = true;
                        for item in items.iter() {
                            if !first {
                                f.write_str(", ")?;
                            }
                            match item {
                                AssignmentPatternItem::KeyValue { key, value } => {
                                    self.with_value(*key).hir_fmt(f)?;
                                    f.write_str(": ")?;
                                    self.with_value(*value).hir_fmt(f)?;
                                }
                                AssignmentPatternItem::Default { value } => {
                                    f.write_str("default: ")?;
                                    self.with_value(*value).hir_fmt(f)?;
                                }
                            }
                            first = false;
                        }
                        f.write_str("}")
                    }
                    AssignmentPattern::Replicated { count, items } => {
                        self.with_value(*count).hir_fmt(f)?;
                        f.write_str("'")?;
                        f.write_str("{")?;
                        let mut first = true;
                        for expr in items.iter() {
                            if !first {
                                f.write_str(", ")?;
                            }
                            self.with_value(*expr).hir_fmt(f)?;
                            first = false;
                        }
                        f.write_str("}}")
                    }
                }
            }
            Expr::Inside { expr, ranges } => {
                self.with_value(*expr).hir_fmt(f)?;
                f.write_str(" inside {")?;
                let mut first = true;
                for range in ranges.iter() {
                    if !first {
                        f.write_str(", ")?;
                    }
                    match range {
                        InsideRange::Expr(expr) => self.with_value(*expr).hir_fmt(f)?,
                        InsideRange::Range { left, right } => {
                            f.write_str("[")?;
                            self.with_value(*left).hir_fmt(f)?;
                            f.write_str(":")?;
                            self.with_value(*right).hir_fmt(f)?;
                            f.write_str("]")?;
                        }
                    }
                    first = false;
                }
                f.write_str("}")
            }
            Expr::Dist { expr, distribution } => {
                self.with_value(*expr).hir_fmt(f)?;
                f.write_str(" dist {")?;
                for (index, item) in distribution.items.iter().enumerate() {
                    if index != 0 {
                        f.write_str(", ")?;
                    }
                    match item {
                        DistItem::Range { range, .. } => match range {
                            InsideRange::Expr(expr) => self.with_value(*expr).hir_fmt(f)?,
                            InsideRange::Range { left, right } => {
                                self.with_value(*left).hir_fmt(f)?;
                                f.write_str(":")?;
                                self.with_value(*right).hir_fmt(f)?;
                            }
                        },
                        DistItem::Default { .. } => f.write_str("default")?,
                    }
                }
                f.write_str("}")
            }
            Expr::TimingControl { expr, .. } => {
                f.write_str("<timing-control> ")?;
                self.with_value(*expr).hir_fmt(f)
            }
            Expr::Binary { op, lhs, rhs } => {
                self.with_value(*lhs).hir_fmt(f)?;
                f.write_str(" ")?;
                op.hir_fmt(f)?;
                f.write_str(" ")?;
                self.with_value(*rhs).hir_fmt(f)
            }
            Expr::Call { callee, args } => {
                self.with_value(*callee).hir_fmt(f)?;
                f.write_str("(")?;

                let mut first = true;
                for arg in args.iter() {
                    if !first {
                        f.write_str(", ")?;
                    }
                    match arg {
                        Arg::Named { name, expr } => {
                            f.write_str(".")?;
                            if let Some(name) = name {
                                f.write_str(name)?;
                            }
                            f.write_str("(")?;
                            self.with_value(*expr).hir_fmt(f)?;
                            f.write_str(")")?;
                        }
                        Arg::Ordered(expr) => {
                            self.with_value(*expr).hir_fmt(f)?;
                        }
                        Arg::Empty => {}
                    }
                    first = false;
                }
                f.write_str(")")
            }
            Expr::NewClass { callee, args } => {
                self.with_value(*callee).hir_fmt(f)?;
                if let Some(args) = args {
                    f.write_str("(")?;
                    let mut first = true;
                    for arg in args.iter() {
                        if !first {
                            f.write_str(", ")?;
                        }
                        match arg {
                            Arg::Named { name, expr } => {
                                f.write_str(".")?;
                                if let Some(name) = name {
                                    f.write_str(name)?;
                                }
                                f.write_str("(")?;
                                self.with_value(*expr).hir_fmt(f)?;
                                f.write_str(")")?;
                            }
                            Arg::Ordered(expr) => {
                                self.with_value(*expr).hir_fmt(f)?;
                            }
                            Arg::Empty => {}
                        }
                        first = false;
                    }
                    f.write_str(")")?;
                }
                Ok(())
            }
            Expr::CopyClass { callee, expr } => {
                self.with_value(*callee).hir_fmt(f)?;
                f.write_str(" ")?;
                self.with_value(*expr).hir_fmt(f)
            }
            Expr::NewArray { size, initializer } => {
                f.write_str("new [")?;
                self.with_value(*size).hir_fmt(f)?;
                f.write_str("]")?;
                if let Some(initializer) = initializer {
                    f.write_str(" (")?;
                    self.with_value(*initializer).hir_fmt(f)?;
                    f.write_str(")")?;
                }
                Ok(())
            }
            Expr::EmptyQueue => f.write_str("{}"),
            Expr::ArrayOrRandomizeMethod { method, with_args, constraints } => {
                self.with_value(*method).hir_fmt(f)?;
                if let Some(args) = with_args {
                    f.write_str(" with (")?;
                    for (index, arg) in args.iter().enumerate() {
                        if index != 0 {
                            f.write_str(", ")?;
                        }
                        self.with_value(*arg).hir_fmt(f)?;
                    }
                    f.write_str(")")?;
                }
                if constraints.is_some() {
                    f.write_str(" { <constraint> }")?;
                }
                Ok(())
            }
            Expr::TaggedUnion { member, expr } => {
                f.write_str("tagged")?;
                if let Some(member) = member {
                    f.write_str(" ")?;
                    f.write_str(member)?;
                }
                if let Some(expr) = expr {
                    f.write_str(" ")?;
                    self.with_value(*expr).hir_fmt(f)?;
                }
                Ok(())
            }
            Expr::SuperNewDefaulted { callee } => {
                self.with_value(*callee).hir_fmt(f)?;
                f.write_str("(default)")
            }
            Expr::Concat(exprs) => {
                f.write_str("{")?;
                let mut first = true;
                for expr in exprs.iter() {
                    if !first {
                        f.write_str(", ")?;
                    }
                    self.with_value(*expr).hir_fmt(f)?;
                    first = false;
                }
                f.write_str("}")
            }
            Expr::Cond { pred, true_expr, false_expr } => {
                self.with_value(*pred).hir_fmt(f)?;
                f.write_str(" ? ")?;
                self.with_value(*true_expr).hir_fmt(f)?;
                f.write_str(" : ")?;
                self.with_value(*false_expr).hir_fmt(f)
            }
            Expr::Field { receiver, field } => {
                self.with_value(*receiver).hir_fmt(f)?;
                f.write_str(".")?;
                if let Some(field) = field { f.write_str(field) } else { f.write_str("<missing>") }
            }
            Expr::Ident(name) => f.write_str(name),
            Expr::Literal(lit) => lit.hir_fmt(f),
            Expr::Cast { ty, expr } => {
                self.with_value(ty.clone()).hir_fmt(f)?;
                f.write_str("'")?;
                f.write_str("(")?;
                self.with_value(*expr).hir_fmt(f)?;
                f.write_str(")")
            }
            Expr::SignedCast { signed, expr } => {
                if *signed {
                    f.write_str("$signed")?;
                } else {
                    f.write_str("$unsigned")?;
                }
                f.write_str("(")?;
                self.with_value(*expr).hir_fmt(f)?;
                f.write_str(")")
            }
            Expr::MinTypMax { min, typ, max } => {
                self.with_value(*min).hir_fmt(f)?;
                f.write_str(":")?;
                self.with_value(*typ).hir_fmt(f)?;
                f.write_str(":")?;
                self.with_value(*max).hir_fmt(f)
            }
            Expr::MultiConcat { concat, rep } => {
                f.write_str("{")?;
                self.with_value(*rep).hir_fmt(f)?;
                f.write_str("{")?;
                let mut first = true;
                for expr in concat.iter() {
                    if !first {
                        f.write_str(", ")?;
                    }
                    self.with_value(*expr).hir_fmt(f)?;
                    first = false;
                }
                f.write_str("}}")
            }
            Expr::PostfixIncDec { op, val } => {
                self.with_value(*val).hir_fmt(f)?;
                match op {
                    IncDecOp::Inc => f.write_str("++"),
                    IncDecOp::Dec => f.write_str("--"),
                }
            }
            Expr::PrefixIncDec { op, val } => {
                match op {
                    IncDecOp::Inc => f.write_str("++")?,
                    IncDecOp::Dec => f.write_str("--")?,
                }
                self.with_value(*val).hir_fmt(f)
            }
            Expr::ElementSelect { receiver, select } => {
                self.with_value(*receiver).hir_fmt(f)?;
                if let Some(select) = select { self.with_value(*select).hir_fmt(f) } else { Ok(()) }
            }
            Expr::Stream { op, slice, concats } => {
                f.write_str("{")?;
                match op {
                    StreamOp::None => {}
                    StreamOp::Right => f.write_str(">>")?,
                    StreamOp::Left => f.write_str("<<")?,
                }
                if let Some(slice) = slice {
                    self.with_value(*slice).hir_fmt(f)?;
                }
                f.write_str("{")?;
                let mut first = true;
                for stream in concats.iter() {
                    if !first {
                        f.write_str(", ")?;
                    }
                    self.with_value(stream.expr).hir_fmt(f)?;
                    if let Some(with_range) = &stream.with_range {
                        f.write_str(" with ")?;
                        if let Some(selector) = with_range.selector {
                            self.with_value(selector).hir_fmt(f)?;
                        }
                    }
                    first = false;
                }
                f.write_str("}}")
            }
            Expr::Unary { op, expr } => {
                op.hir_fmt(f)?;
                self.with_value(*expr).hir_fmt(f)
            }
            Expr::Sequence(sequence) => fmt_sequence_expr(self, f, sequence),
            Expr::Property(property) => fmt_property_expr(self, f, property),
        }
    }
}

impl HirDisplay for NetKind {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        match self {
            NetKind::Supply0 => f.write_str("supply0"),
            NetKind::Supply1 => f.write_str("supply1"),
            NetKind::Tri => f.write_str("tri"),
            NetKind::Triand => f.write_str("triand"),
            NetKind::Trior => f.write_str("trior"),
            NetKind::Tri0 => f.write_str("tri0"),
            NetKind::Tri1 => f.write_str("tri1"),
            NetKind::Wire => f.write_str("wire"),
            NetKind::Wand => f.write_str("wand"),
            NetKind::Wor => f.write_str("wor"),
            NetKind::Uwire => f.write_str("uwire"),
        }
    }
}

impl HirDisplay for TimeUnit {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        match self {
            TimeUnit::Seconds => f.write_str("s"),
            TimeUnit::Milliseconds => f.write_str("ms"),
            TimeUnit::Microseconds => f.write_str("us"),
            TimeUnit::Nanoseconds => f.write_str("ns"),
            TimeUnit::Picoseconds => f.write_str("ps"),
            TimeUnit::Femtoseconds => f.write_str("fs"),
        }
    }
}

impl HirDisplay for Literal {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        match self {
            Literal::Int(i) => f.write_str(&i.serialize(10)),
            Literal::Float(wrapper) => f.write_str(&format!("{:?}", f64::from(*wrapper))),
            Literal::Time { val, unit } => {
                f.write_str(&format!("{:?}", f64::from(*val)))?;
                unit.hir_fmt(f)
            }
            Literal::Str(s) => f.write_str(s),
            Literal::UnbasedUnsized(bit) => f.write_str(&format!("{bit}")),
            Literal::Null => f.write_str("null"),
            Literal::Unbounded => f.write_str("$"),
        }
    }
}

impl HirDisplay for OwnerRef<Dimension> {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        f.write_char('[')?;
        match self.value {
            Dimension::Range(start, end) => {
                self.with_value(start).hir_fmt(f)?;
                f.write_str(":")?;
                self.with_value(end).hir_fmt(f)?;
            }
            Dimension::Size(idx) => self.with_value(idx).hir_fmt(f)?,
            Dimension::Queue(size) => {
                f.write_str("$")?;
                if let Some(size) = size {
                    f.write_str(":")?;
                    self.with_value(size).hir_fmt(f)?;
                }
            }
            Dimension::Assoc(key) => self.with_value(key).hir_fmt(f)?,
            Dimension::Wildcard => f.write_str("*")?,
            Dimension::Dynamic => {}
        }
        f.write_char(']')
    }
}

impl HirDisplay for OwnerRef<DeclId> {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        let OwnerRef { cont_id, value: decl_id } = self;
        let container = cont_id.data(f.db);
        let decl = container.declarator(*decl_id);

        if let Some(name) = &decl.name {
            f.write_str(name)?;
        }

        for dim in decl.dimensions.iter().flatten() {
            self.with_value(*dim).hir_fmt(f)?;
        }

        Ok(())
    }
}

impl HirDisplay for OwnerRef<TypedefId> {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        let OwnerRef { cont_id, value: typedef_id } = self;
        let container = cont_id.data(f.db);
        let typedef = container.typedef(*typedef_id);

        f.write_str("typedef ")?;
        if let Some(ty) = typedef.ty.clone() {
            OwnerRef::new(*cont_id, ty).hir_fmt(f)?;
            if typedef.name.is_some() {
                f.write_str(" ")?;
            }
        }

        if let Some(name) = &typedef.name {
            f.write_str(name)?;
        }

        Ok(())
    }
}

impl HirDisplay for OwnerRef<Selector> {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        f.write_char('[')?;
        match self.value {
            Selector::Bit(idx) => {
                self.with_value(idx).hir_fmt(f)?;
            }
            Selector::Range(left, right) => {
                self.with_value(left).hir_fmt(f)?;
                f.write_str(":")?;
                self.with_value(right).hir_fmt(f)?;
            }
            Selector::Ascending(left, right) => {
                self.with_value(left).hir_fmt(f)?;
                f.write_str("+:")?;
                self.with_value(right).hir_fmt(f)?;
            }
            Selector::Descending(left, right) => {
                self.with_value(left).hir_fmt(f)?;
                f.write_str("-:")?;
                self.with_value(right).hir_fmt(f)?;
            }
        }
        f.write_str("]")
    }
}

#[cfg(test)]
mod tests;
