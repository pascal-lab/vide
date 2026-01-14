use std::fmt::{self, Debug};

use base_db::intern::Lookup;
use syntax::TimeUnit;
use triomphe::Arc;
use utils::get::GetRef;

use crate::{
    container::{ContainerId, InContainer, InModule},
    db::HirDb,
    hir_def::{
        aggregate::StructKind,
        expr::{
            Arg, AssignOp, BinaryOp, Expr, ExprId, IncDecOp, Selector, StreamOp, UnaryOp,
            data_ty::{BuiltinDataTy, DataTy, Dimension, IntKind, NamedDataTy, Real, VecKind},
            declarator::DeclId,
        },
        literal::Literal,
        module::port::{PortDirection, PortHeader},
        ty::{NetKind, NetType},
    },
};

pub struct HirFormatter<'a> {
    pub db: &'a dyn HirDb,
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

#[derive(Debug)]
pub struct HirDisplayError(fmt::Error);

impl From<fmt::Error> for HirDisplayError {
    fn from(err: fmt::Error) -> Self {
        HirDisplayError(err)
    }
}

pub trait HirDisplay {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError>;

    fn display_source(&self, db: &dyn HirDb) -> Result<String, HirDisplayError> {
        let mut res = String::new();
        self.hir_fmt(&mut HirFormatter { db, f: &mut res, simplified_ty: false })?;
        Ok(res)
    }

    fn display_signature(&self, db: &dyn HirDb) -> Result<String, HirDisplayError> {
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

impl HirDisplay for InContainer<DataTy> {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        match self.value {
            DataTy::Builtin(ty_id) => match ty_id.lookup(f.db) {
                BuiltinDataTy::Int { kind, signing } => {
                    if signing {
                        f.write_str("signed ")?;
                    }
                    match kind {
                        IntKind::Byte => f.write_str("byte"),
                        IntKind::ShortInt => f.write_str("shortint"),
                        IntKind::Int => f.write_str("int"),
                        IntKind::LongInt => f.write_str("longint"),
                        IntKind::Integer => f.write_str("integer"),
                        IntKind::Time => f.write_str("time"),
                    }
                }
                BuiltinDataTy::Vector { kind, signing, dimensions } => {
                    if signing {
                        f.write_str("signed ")?;
                    }
                    match kind {
                        VecKind::Bit => {
                            if !f.simplified_ty {
                                f.write_str("bit")?
                            }
                        }
                        VecKind::Logic => {
                            if !f.simplified_ty {
                                f.write_str("logic")?
                            }
                        }
                        VecKind::Reg => f.write_str("reg")?,
                    }
                    for dim in dimensions.iter().flatten() {
                        self.with_value(*dim).hir_fmt(f)?;
                    }
                    Ok(())
                }
                BuiltinDataTy::Real(real) => match real {
                    Real::Real => f.write_str("real"),
                    Real::ShortReal => f.write_str("shortreal"),
                    Real::RealTime => f.write_str("realtime"),
                },
                BuiltinDataTy::String => f.write_str("string"),
                BuiltinDataTy::Void => f.write_str("void"),
            },
            DataTy::Named(named) => match named {
                NamedDataTy::Ident(expr_id) => self.with_value(expr_id).hir_fmt(f),
                NamedDataTy::Field(expr_id) => self.with_value(expr_id).hir_fmt(f),
            },
            DataTy::Struct(struct_ref) => {
                let cont = struct_ref.cont_id.to_container(f.db);
                let def = cont.get(struct_ref.value);
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
        }
    }
}

impl HirDisplay for InModule<PortHeader> {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        let InModule { module_id, value: port_header } = self;
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
                InContainer::new((*module_id).into(), *ty).hir_fmt(f)
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
                InContainer::new((*module_id).into(), *ty).hir_fmt(f)
            }
        }
    }
}

impl HirDisplay for InContainer<ExprId> {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        let InContainer { cont_id, value: expr_id } = self;
        let container = cont_id.to_container(f.db);
        let expr = container.get(*expr_id);
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

impl HirDisplay for InContainer<&Expr> {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        match self.value {
            Expr::Missing => f.write_str(""),
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
                self.with_value(*ty).hir_fmt(f)?;
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
                let mut first = true;
                for expr in concats.iter() {
                    if !first {
                        f.write_str(", ")?;
                    }
                    self.with_value(*expr).hir_fmt(f)?;
                    first = false;
                }
                f.write_str("}")
            }
            Expr::Unary { op, expr } => {
                op.hir_fmt(f)?;
                self.with_value(*expr).hir_fmt(f)
            }
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
        }
    }
}

impl HirDisplay for InContainer<Dimension> {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        f.write_char('[')?;
        match self.value {
            Dimension::Range(start, end) => {
                self.with_value(start).hir_fmt(f)?;
                f.write_str(":")?;
                self.with_value(end).hir_fmt(f)?;
            }
            Dimension::Size(idx) => self.with_value(idx).hir_fmt(f)?,
        }
        f.write_char(']')
    }
}

impl HirDisplay for InContainer<DeclId> {
    fn hir_fmt(&self, f: &mut HirFormatter<'_>) -> Result<(), HirDisplayError> {
        let InContainer { cont_id, value: decl_id } = self;
        let container = cont_id.to_container(f.db);
        let decl = container.get(*decl_id);

        if let Some(name) = &decl.name {
            f.write_str(name)?;
        }

        for dim in decl.dimensions.iter().flatten() {
            self.with_value(*dim).hir_fmt(f)?;
        }

        Ok(())
    }
}

impl HirDisplay for InContainer<Selector> {
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
