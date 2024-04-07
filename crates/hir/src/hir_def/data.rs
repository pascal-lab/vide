use crate::hir_def::{
    expr::{ExprId, LowerExpr, MinTypMaxExpr},
    module::port::{AnsiPortDecl, PortDecl},
    try_match, Ident, InFile, SourceMap,
};
use la_arena::{Arena, Idx, IdxRange, RawIdx};
use smallvec::SmallVec;
use syntax::ast::{self, ptr};
use utils::try_;

use super::literal::Literal;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum DataType {
    Implicit { dimensions: Option<SmallVec<[Dimension; 1]>>, sign: bool },
    IntegerType(IntegerType),
    NonIntegerType,
    StructUnion,
    Enum,
    String,
    // TODO: for paramdecl syntax:
    //      parameter_declaration ::= parameter type list_of_type_assignments
    // TODO: complete all the data types
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum IntegerType {
    Byte { sign: bool },
    ShortInt { sign: bool },
    Int { sign: bool },
    LongInt { sign: bool },
    Integer { sign: bool },
    Time { sign: bool },
    Bit { dimensions: Option<SmallVec<[Dimension; 1]>>, sign: bool },
    Logic { dimensions: Option<SmallVec<[Dimension; 1]>>, sign: bool },
    Reg { dimensions: Option<SmallVec<[Dimension; 1]>>, sign: bool },
}

pub(crate) fn lower_signing(signing: &ast::Signing) -> Option<bool> {
    try_match! {
        signing.token_signed(), _ => Some(true),
        signing.token_unsigned(), _ => Some(false),
        _ => None,
    }
}

pub(crate) trait LowerDataType: LowerDimension {
    fn lower_data_type(&mut self, data_type: &ast::DataType) -> Option<DataType> {
        try_match! {
            // 6.11
            data_type.integer_atom_type(), int_atom => try_!{
                let sign = try_match!{
                    data_type.signing(), signing => lower_signing(&signing)?,
                    _ => true,
                };
                DataType::IntegerType(try_match!{
                    int_atom.token_byte(), _ => Some(IntegerType::Byte{sign}),
                    int_atom.token_shortint(), _ => Some(IntegerType::ShortInt{sign}),
                    int_atom.token_int(), _ => Some(IntegerType::Int{sign}),
                    int_atom.token_longint(), _ => Some(IntegerType::LongInt{sign}),
                    int_atom.token_integer(), _ => Some(IntegerType::Integer{sign}),
                    int_atom.token_time(), _ => Some(IntegerType::Time{sign}),
                    _ => None,
                }?)
            },
            // 6.11
            data_type.integer_vector_type(), int_vector => try_!{
                let sign = try_match!{
                    data_type.signing(), signing => lower_signing(&signing)?,
                    _ => false,
                };
                let mut dimensions: SmallVec<[Dimension; 1]> = SmallVec::new();
                for packed_dimension in data_type.packed_dimensions() {
                    dimensions.push(self.lower_packed_dimension(&packed_dimension)?);
                }
                let dimensions = if dimensions.is_empty() { None } else { Some(dimensions) };
                DataType::IntegerType(try_match!{
                    int_vector.token_bit(), _ => Some(IntegerType::Bit{dimensions, sign}),
                    int_vector.token_logic(), _ => Some(IntegerType::Logic{dimensions, sign}),
                    int_vector.token_reg(), _ => Some(IntegerType::Reg{dimensions, sign}),
                    _ => None,
                }?)
            },
            _ => unimplemented!("Lower DataType")
        }
    }

    fn lower_data_type_or_implicit(
        &mut self,
        data_type_or_implicit: &ast::DataTypeOrImplicit,
    ) -> Option<DataType> {
        try_match! {
            data_type_or_implicit.data_type(), data_type => {
                self.lower_data_type(&data_type)
            },
            data_type_or_implicit.implicit_data_type(), implicit_data_type => {
                Some(DataType::Implicit{
                    dimensions: {
                        let mut dimensions: SmallVec<[Dimension; 1]> = SmallVec::new();
                        for packed_dimension in implicit_data_type.packed_dimensions() {
                            dimensions.push(self.lower_packed_dimension(&packed_dimension)?);
                        }
                        if dimensions.is_empty() { None } else { Some(dimensions) }
                    },
                    sign:try_match!{
                        implicit_data_type.signing(), signing => lower_signing(&signing)?,
                        _ => false,
                    }
                })
            },
            _ => None
        }
    }
}

// TODO: associative_dimension | queue_dimension | Unsized
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Dimension {
    Range(ExprId, ExprId),
    Expr(ExprId),
    // Unsized,
}

pub(crate) trait LowerDimension: LowerExpr {
    fn lower_packed_dimension(
        &mut self,
        packed_dimension: &ast::PackedDimension,
    ) -> Option<Dimension> {
        try_match! {
            packed_dimension.constant_range(), range => {
                let left_expr_node = range.constant_expressions().next()?;
                let right_expr_node = range.constant_expressions().next()?;
                Some(Dimension::Range(
                    self.lower_const_expr(&left_expr_node),
                    self.lower_const_expr(&right_expr_node),
                ))
            },
            // TODO: Unsized
            _ => unimplemented!("Packed Dimension")
        }
    }

    fn lower_unpacked_dimension(
        &mut self,
        unpacked_dimension: &ast::UnpackedDimension,
    ) -> Option<Dimension> {
        try_match! {
            unpacked_dimension.constant_range(), range => {
                let left_expr_node = range.constant_expressions().next()?;
                let right_expr_node = range.constant_expressions().next()?;
                Some(Dimension::Range(
                    self.lower_const_expr(&left_expr_node),
                    self.lower_const_expr(&right_expr_node),
                ))
            },
            unpacked_dimension.constant_expression(), expr => {
                Some(Dimension::Expr(self.lower_const_expr(&expr)))
            },
            _ => None
        }
    }

    fn lower_var_dimension(&mut self, var_dimension: &ast::VariableDimension) -> Option<Dimension> {
        try_match! {
            var_dimension.unpacked_dimension(), unpacked => {
                self.lower_unpacked_dimension(&unpacked)
            },
            var_dimension.associative_dimension(), _associative => {
                unimplemented!("Associative Dimension");
            },
            var_dimension.queue_dimension(), _queue => {
                unimplemented!("Queue Dimension");
            },
            var_dimension.unsized_dimension(), _unsized => {
                unimplemented!("Unsized Dimension");
            },
            _ => None
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum NetType {
    Supply0,
    Supply1,
    Tri,
    Triand,
    Trior,
    Tri0,
    Tri1,
    Wire,
    Wand,
    Wor,
    Uwire,
}

pub(crate) fn lower_net_type(net_type: &ast::NetType) -> Option<NetType> {
    try_match! {
        net_type.token_supply0(), _ => Some(NetType::Supply0),
        net_type.token_supply1(), _ => Some(NetType::Supply1),
        net_type.token_tri(), _ => Some(NetType::Tri),
        net_type.token_triand(), _ => Some(NetType::Triand),
        net_type.token_trior(), _ => Some(NetType::Trior),
        net_type.token_tri0(), _ => Some(NetType::Tri0),
        net_type.token_tri1(), _ => Some(NetType::Tri1),
        net_type.token_wire(), _ => Some(NetType::Wire),
        net_type.token_wand(), _ => Some(NetType::Wand),
        net_type.token_wor(), _ => Some(NetType::Wor),
        net_type.token_uwire(), _ => Some(NetType::Uwire),
        _ => None
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum NetKind {
    Default { net_type: NetType, data_type: DataType },
    // TODO: net_type_identifier
    // Ident{ident: Ident},
}

pub const DEFAULT_NET_TYPE: NetType = NetType::Wire;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct DataSubDecl {
    pub ident: Ident,
    pub dimensions: Option<SmallVec<[Dimension; 1]>>,
    pub expr: Option<ExprId>,
    pub full_decl: DataFullDecl,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum DataFullDecl {
    DataDecl(Idx<DataDecl>),
    AnsiPortDecl(Idx<AnsiPortDecl>),
    PortDecl(Idx<PortDecl>),
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum LocalDataSubDeclSrc {
    NetDeclAssign(ptr::NetDeclAssignmentPtr),
    VarDeclAssign(ptr::VariableDeclAssignmentPtr),
    ParamAssign(ptr::ParamAssignmentPtr),
    AnsiPortDecl(ptr::AnsiPortDeclarationPtr),
    // Those SubDecls Below is edited for convenience
    PortIdentDecl(ptr::PortIdentifierDeclarationPtr),
    VarIdentDecl(ptr::VariableIdentifierDeclarationPtr),
    VarPortIdentDecl(ptr::VariablePortIdentifierDeclarationPtr),
}

pub type DataSubDeclSrc = InFile<LocalDataSubDeclSrc>;

pub(crate) trait LowerDataSubDecl: LowerDimension + LowerExpr {
    fn arena_data_sub_decl(&mut self) -> &mut Arena<DataSubDecl>;

    fn src_map_data_sub_decl(&mut self) -> &mut SourceMap<DataSubDeclSrc, DataSubDecl>;

    fn next_data_sub_decl_idx(&mut self) -> Idx<DataSubDecl> {
        Idx::from_raw(RawIdx::from(self.arena_data_sub_decl().len() as u32))
    }

    fn lower_net_sub_decl(
        &mut self,
        net_assign: &ast::NetDeclAssignment,
        full_decl: Idx<DataDecl>,
    ) -> Option<Idx<DataSubDecl>> {
        let ident = self.lower_ident(&net_assign.identifier()?)?;
        let expr = net_assign.expression().map(|expr| self.lower_expr(&expr));
        let mut dimensions = SmallVec::<[Dimension; 1]>::new();
        for unpacked_dimension in net_assign.unpacked_dimensions() {
            dimensions.push(self.lower_unpacked_dimension(&unpacked_dimension)?);
        }
        let dimensions = if dimensions.is_empty() { None } else { Some(dimensions) };
        let src = self.in_file(LocalDataSubDeclSrc::NetDeclAssign(net_assign.to_ptr()));
        let full_decl = DataFullDecl::DataDecl(full_decl);
        let idx =
            self.arena_data_sub_decl().alloc(DataSubDecl { ident, dimensions, expr, full_decl });
        self.src_map_data_sub_decl().insert(src, idx);
        Some(idx)
    }

    fn lower_net_sub_decl_list(
        &mut self,
        net_decl_list: &ast::ListOfNetDeclAssignment,
        full_decl: Idx<DataDecl>,
    ) -> IdxRange<DataSubDecl> {
        let begin_idx = self.next_data_sub_decl_idx();
        for net_decl in net_decl_list.net_decl_assignments() {
            self.lower_net_sub_decl(&net_decl, full_decl);
        }
        let end_idx = self.next_data_sub_decl_idx();
        IdxRange::new(begin_idx..end_idx)
    }

    fn lower_var_sub_decl(
        &mut self,
        var_assign: &ast::VariableDeclAssignment,
        full_decl: Idx<DataDecl>,
    ) -> Option<Idx<DataSubDecl>> {
        let ident = self.lower_ident(&var_assign.identifier()?)?;
        let expr = var_assign.expression().map(|expr| self.lower_expr(&expr));
        let mut dimensions = SmallVec::<[Dimension; 1]>::new();
        for var_dimension in var_assign.variable_dimensions() {
            dimensions.push(self.lower_var_dimension(&var_dimension)?);
        }
        let dimensions = if dimensions.is_empty() { None } else { Some(dimensions) };
        let src = self.in_file(LocalDataSubDeclSrc::VarDeclAssign(var_assign.to_ptr()));
        let full_decl = DataFullDecl::DataDecl(full_decl);
        let idx =
            self.arena_data_sub_decl().alloc(DataSubDecl { ident, dimensions, expr, full_decl });
        self.src_map_data_sub_decl().insert(src, idx);
        Some(idx)
    }

    fn lower_var_sub_decl_list(
        &mut self,
        var_decl_list: &ast::ListOfVariableDeclAssignment,
        full_decl: Idx<DataDecl>,
    ) -> IdxRange<DataSubDecl> {
        let begin_idx = self.next_data_sub_decl_idx();
        for var_decl in var_decl_list.variable_decl_assignments() {
            self.lower_var_sub_decl(&var_decl, full_decl);
        }
        let end_idx = self.next_data_sub_decl_idx();
        IdxRange::new(begin_idx..end_idx)
    }

    fn lower_param_sub_decl(
        &mut self,
        param_assign: &ast::ParamAssignment,
        full_decl: Idx<DataDecl>,
    ) -> Option<Idx<DataSubDecl>> {
        let ident = self.lower_ident(&param_assign.identifier()?)?;
        let expr = param_assign
            .constant_param_expression()
            .and_then(|expr| self.lower_const_param_expr(&expr));
        let mut dimensions = SmallVec::<[Dimension; 1]>::new();
        for unpacked_dimension in param_assign.unpacked_dimensions() {
            dimensions.push(self.lower_unpacked_dimension(&unpacked_dimension)?);
        }
        let dimensions = if dimensions.is_empty() { None } else { Some(dimensions) };
        let src = self.in_file(LocalDataSubDeclSrc::ParamAssign(param_assign.to_ptr()));
        let full_decl = DataFullDecl::DataDecl(full_decl);
        let idx =
            self.arena_data_sub_decl().alloc(DataSubDecl { ident, dimensions, expr, full_decl });
        self.src_map_data_sub_decl().insert(src, idx);
        Some(idx)
    }

    fn lower_param_sub_decl_list(
        &mut self,
        param_decl_list: &ast::ListOfParamAssignment,
        full_decl: Idx<DataDecl>,
    ) -> IdxRange<DataSubDecl> {
        let begin_idx = self.next_data_sub_decl_idx();
        for param_decl in param_decl_list.param_assignments() {
            self.lower_param_sub_decl(&param_decl, full_decl);
        }
        let end_idx = self.next_data_sub_decl_idx();
        IdxRange::new(begin_idx..end_idx)
    }

    fn lower_ansi_port_decl(
        &mut self,
        ansi_port_decl: &ast::AnsiPortDeclaration,
        full_decl: Idx<AnsiPortDecl>,
    ) -> Option<Idx<DataSubDecl>> {
        let ident = self.lower_ident(&ansi_port_decl.identifier()?)?;
        let expr = ansi_port_decl
            .constant_expression()
            .map(|const_expr| self.lower_const_expr(&const_expr));
        let mut dimensions = SmallVec::<[Dimension; 1]>::new();
        for unpacked_dimension in ansi_port_decl.unpacked_dimensions() {
            dimensions.push(self.lower_unpacked_dimension(&unpacked_dimension)?);
        }
        for var_dimension in ansi_port_decl.variable_dimensions() {
            dimensions.push(self.lower_var_dimension(&var_dimension)?);
        }
        let dimensions = if dimensions.is_empty() { None } else { Some(dimensions) };
        let src = self.in_file(LocalDataSubDeclSrc::AnsiPortDecl(ansi_port_decl.to_ptr()));
        let full_decl = DataFullDecl::AnsiPortDecl(full_decl);
        let idx =
            self.arena_data_sub_decl().alloc(DataSubDecl { ident, dimensions, expr, full_decl });
        self.src_map_data_sub_decl().insert(src, idx);
        Some(idx)
    }

    fn lower_port_ident_decl(
        &mut self,
        port_ident_decl: &ast::PortIdentifierDeclaration,
        full_decl: Idx<PortDecl>,
    ) -> Option<Idx<DataSubDecl>> {
        let ident = self.lower_ident(&port_ident_decl.identifier()?)?;
        let mut dimensions: SmallVec<[Dimension; 1]> = SmallVec::new();
        for packed_dimension in port_ident_decl.unpacked_dimensions() {
            dimensions.push(self.lower_unpacked_dimension(&packed_dimension)?);
        }
        let dimensions = if dimensions.is_empty() { None } else { Some(dimensions) };
        let src = self.in_file(LocalDataSubDeclSrc::PortIdentDecl(port_ident_decl.to_ptr()));
        let full_decl = DataFullDecl::PortDecl(full_decl);
        let idx = self.arena_data_sub_decl().alloc(DataSubDecl {
            ident,
            dimensions,
            expr: None,
            full_decl,
        });
        self.src_map_data_sub_decl().insert(src, idx);
        Some(idx)
    }

    fn lower_port_ident_list(
        &mut self,
        port_ident_list: &ast::ListOfPortIdentifier,
        full_decl: Idx<PortDecl>,
    ) -> IdxRange<DataSubDecl> {
        let begin_idx = self.next_data_sub_decl_idx();
        for port_ident_decl in port_ident_list.port_identifier_declarations() {
            self.lower_port_ident_decl(&port_ident_decl, full_decl);
        }
        let end_idx = self.next_data_sub_decl_idx();
        IdxRange::new(begin_idx..end_idx)
    }

    fn lower_var_ident_decl(
        &mut self,
        var_ident_decl: &ast::VariableIdentifierDeclaration,
        full_decl: Idx<PortDecl>,
    ) -> Option<Idx<DataSubDecl>> {
        let ident = self.lower_ident(&var_ident_decl.identifier()?)?;
        let mut dimensions: SmallVec<[Dimension; 1]> = SmallVec::new();
        for packed_dimension in var_ident_decl.variable_dimensions() {
            dimensions.push(self.lower_var_dimension(&packed_dimension)?);
        }
        let dimensions = if dimensions.is_empty() { None } else { Some(dimensions) };
        let src = self.in_file(LocalDataSubDeclSrc::VarIdentDecl(var_ident_decl.to_ptr()));
        let full_decl = DataFullDecl::PortDecl(full_decl);
        let idx = self.arena_data_sub_decl().alloc(DataSubDecl {
            ident,
            dimensions,
            expr: None,
            full_decl,
        });
        self.src_map_data_sub_decl().insert(src, idx);
        Some(idx)
    }

    fn lower_var_ident_list(
        &mut self,
        var_ident_list: &ast::ListOfVariableIdentifier,
        full_decl: Idx<PortDecl>,
    ) -> IdxRange<DataSubDecl> {
        let begin_idx = self.next_data_sub_decl_idx();
        for var_ident_decl in var_ident_list.variable_identifier_declarations() {
            self.lower_var_ident_decl(&var_ident_decl, full_decl);
        }
        let end_idx = self.next_data_sub_decl_idx();
        IdxRange::new(begin_idx..end_idx)
    }

    fn lower_var_port_ident_decl(
        &mut self,
        var_port_ident_decl: &ast::VariablePortIdentifierDeclaration,
        full_decl: Idx<PortDecl>,
    ) -> Option<Idx<DataSubDecl>> {
        let ident = self.lower_ident(&var_port_ident_decl.identifier()?)?;
        let expr = var_port_ident_decl
            .constant_expression()
            .map(|const_expr| self.lower_const_expr(&const_expr));
        let mut dimensions: SmallVec<[Dimension; 1]> = SmallVec::new();
        for packed_dimension in var_port_ident_decl.variable_dimensions() {
            dimensions.push(self.lower_var_dimension(&packed_dimension)?);
        }
        let dimensions = if dimensions.is_empty() { None } else { Some(dimensions) };
        let src = self.in_file(LocalDataSubDeclSrc::VarPortIdentDecl(var_port_ident_decl.to_ptr()));
        let full_decl = DataFullDecl::PortDecl(full_decl);
        let idx =
            self.arena_data_sub_decl().alloc(DataSubDecl { ident, dimensions, expr, full_decl });
        self.src_map_data_sub_decl().insert(src, idx);
        Some(idx)
    }

    fn lower_var_port_ident_list(
        &mut self,
        var_port_ident_list: &ast::ListOfVariablePortIdentifier,
        full_decl: Idx<PortDecl>,
    ) -> IdxRange<DataSubDecl> {
        let begin_idx = self.next_data_sub_decl_idx();
        for var_port_ident_decl in var_port_ident_list.variable_port_identifier_declarations() {
            self.lower_var_port_ident_decl(&var_port_ident_decl, full_decl);
        }
        let end_idx = self.next_data_sub_decl_idx();
        IdxRange::new(begin_idx..end_idx)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum ChargeStrength {
    Small,
    Medium,
    Large,
}

pub(crate) fn lower_charge_strength(
    charge_strength: &ast::ChargeStrength,
) -> Option<ChargeStrength> {
    try_match! {
        charge_strength.token_small(), _ => Some(ChargeStrength::Small),
        charge_strength.token_medium(), _ => Some(ChargeStrength::Medium),
        charge_strength.token_large(), _ => Some(ChargeStrength::Large),
        _ => None
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum DriveSubStrength {
    Supply,
    Strong,
    Pull,
    Weak,
    Highz,
}

pub type DriveStrength = (DriveSubStrength, DriveSubStrength);

pub(crate) fn lower_drive_strength(drive_strength: &ast::DriveStrength) -> Option<DriveStrength> {
    let strength0 = try_match! {
        drive_strength.strength0(), strength0 => {
            try_match! {
                strength0.token_supply0(), _ => DriveSubStrength::Supply,
                strength0.token_strong0(), _ => DriveSubStrength::Strong,
                strength0.token_pull0(), _ => DriveSubStrength::Pull,
                strength0.token_weak0(), _ => DriveSubStrength::Weak,
                _ => { return None; }
            }
        },
        drive_strength.token_highz0(), _ => DriveSubStrength::Highz,
        _ => { return None; }
    };
    let strength1 = try_match! {
        drive_strength.strength1(), strength1 => {
            try_match! {
                strength1.token_supply1(), _ => DriveSubStrength::Supply,
                strength1.token_strong1(), _ => DriveSubStrength::Strong,
                strength1.token_pull1(), _ => DriveSubStrength::Pull,
                strength1.token_weak1(), _ => DriveSubStrength::Weak,
                _ => { return None; }
            }
        },
        drive_strength.token_highz1(), _ => DriveSubStrength::Highz,
        _ => { return None; }
    };
    Some((strength0, strength1))
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Delay {
    Val(Literal),
    MinTyMax(SmallVec<[MinTypMaxExpr; 3]>),
}

pub(crate) trait LowerDelay: LowerExpr {
    fn lower_delay3(&mut self, delay3: &ast::Delay3) -> Option<Delay> {
        try_match! {
            delay3.delay_value(), delay_value => Some(Delay::Val(self.lower_delay_value(&delay_value)?)),
            _ => {
                let min_ty_max = delay3.mintypmax_expressions().map(|expr| self.lower_min_typ_max_expr(&expr)).collect::<SmallVec<_>>();
                Some(Delay::MinTyMax(min_ty_max))
            }
        }
    }

    fn lower_delay2(&mut self, delay2: &ast::Delay2) -> Option<Delay> {
        try_match! {
            delay2.delay_value(), delay_value => Some(Delay::Val(self.lower_delay_value(&delay_value)?)),
            _ => {
                let min_ty_max = delay2.mintypmax_expressions().map(|expr| self.lower_min_typ_max_expr(&expr)).collect::<SmallVec<_>>();
                Some(Delay::MinTyMax(min_ty_max))
            }
        }
    }
}

// Todo: [ drive_strength | charge_strength ]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct NetDecl {
    pub net_kind: NetKind,
    pub drive_strength: Option<DriveStrength>,
    pub charge_strength: Option<ChargeStrength>,
    pub vectored: bool,
    pub scalared: bool,
    pub delay: Option<Delay>,
    pub sub_decls: IdxRange<DataSubDecl>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct VarDecl {
    // TODO: lifetime
    pub konst: bool,
    pub data_type: DataType,
    pub sub_decls: IdxRange<DataSubDecl>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ParamDecl {
    pub local: bool,
    // 6.20.2
    pub data_type: Option<DataType>,
    pub sub_decls: IdxRange<DataSubDecl>,
}

// TODO: TypeDecl, NetTypeDecl, package_import_declaration11, event
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum DataDecl {
    NetDecl(NetDecl),
    VarDecl(VarDecl),
    ParamDecl(ParamDecl),
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum LocalDataDeclSrc {
    NetDecl(ptr::NetDeclarationPtr),
    VarDecl(ptr::VariableDeclarationPtr),
    ParamDecl(ptr::ParameterDeclarationPtr),
    LocalParamDecl(ptr::LocalParameterDeclarationPtr),
    ParamAssignList(ptr::ListOfParamAssignmentPtr),
    ParamPortDecl(ptr::ParameterPortDeclarationPtr),
}

pub type DataDeclSrc = InFile<LocalDataDeclSrc>;

pub(crate) trait LowerDataDecl: LowerDataSubDecl + LowerDataType + LowerDelay {
    fn arena_data_decl(&mut self) -> &mut Arena<DataDecl>;

    fn src_map_data_decl(&mut self) -> &mut SourceMap<DataDeclSrc, DataDecl>;

    fn next_data_decl_idx(&mut self) -> Idx<DataDecl> {
        Idx::from_raw(RawIdx::from(self.arena_data_decl().len() as u32))
    }

    fn lower_net_decl(&mut self, net_decl: &ast::NetDeclaration) -> Option<Idx<DataDecl>> {
        try_match! {
            net_decl.net_type(), net_type => {
                let net_type = lower_net_type(&net_type)?;
                let data_type = {
                    let data_type = net_decl.data_type_or_implicit()?;
                    self.lower_data_type_or_implicit(&data_type)?
                };
                let src = self.in_file(LocalDataDeclSrc::NetDecl(net_decl.to_ptr()));
                let idx = self.next_data_decl_idx();
                let data_decl = DataDecl::NetDecl(NetDecl {
                    net_kind: NetKind::Default{net_type, data_type},
                    drive_strength: net_decl.drive_strength().and_then(|drive_strength| lower_drive_strength(&drive_strength)),
                    charge_strength: net_decl.charge_strength().and_then(|charge_strength| lower_charge_strength(&charge_strength)),
                    vectored: net_decl.token_vectored().is_some(),
                    scalared: net_decl.token_scalared().is_some(),
                    delay: net_decl.delay3().and_then(|delay3| self.lower_delay3(&delay3)),
                    sub_decls: self.lower_net_sub_decl_list(&net_decl.list_of_net_decl_assignments()?, idx),
                });
                self.arena_data_decl().alloc(data_decl);
                self.src_map_data_decl().insert(src, idx);
                Some(idx)
            },
            net_decl.net_declaration_with_net_type_identifier(), _net_decl_with_net_type_identifier => {
                unimplemented!("net_declaration ::= net_type_identifier [delay_control]
                list_of_net_decl_assignments;")
            },
            net_decl.token_interconnect(), _ => {
                unimplemented!("net_declaration ::= interconnect implicit_data_type [#delay_value] net_identifier {{unpacked_dimension}}
                [, net_identifier {{unpacked_dimension}}];")
            },
            _ => None
        }
    }

    fn lower_var_decl(&mut self, var_decl: &ast::VariableDeclaration) -> Option<Idx<DataDecl>> {
        let src = self.in_file(LocalDataDeclSrc::VarDecl(var_decl.to_ptr()));
        let idx = self.next_data_decl_idx();
        let data_decl = DataDecl::VarDecl(VarDecl {
            konst: var_decl.token_const().is_some(),
            data_type: {
                let data_type = var_decl.data_type_or_implicit()?;
                self.lower_data_type_or_implicit(&data_type)?
            },
            sub_decls: self
                .lower_var_sub_decl_list(&var_decl.list_of_variable_decl_assignments()?, idx),
        });
        self.arena_data_decl().alloc(data_decl);
        self.src_map_data_decl().insert(src, idx);
        Some(idx)
    }

    fn lower_param_decl(
        &mut self,
        param_decl: &ast::ParameterDeclaration,
    ) -> Option<Idx<DataDecl>> {
        if param_decl.token_type().is_some() {
            unimplemented!("Parameter Type");
        } else {
            let src = self.in_file(LocalDataDeclSrc::ParamDecl(param_decl.to_ptr()));
            let idx = self.next_data_decl_idx();
            let data_decl = DataDecl::ParamDecl(ParamDecl {
                local: false,
                data_type: {
                    let data_type = param_decl.data_type_or_implicit()?;
                    Some(self.lower_data_type_or_implicit(&data_type)?)
                },
                sub_decls: self
                    .lower_param_sub_decl_list(&param_decl.list_of_param_assignments()?, idx),
            });
            self.arena_data_decl().alloc(data_decl);
            self.src_map_data_decl().insert(src, idx);
            Some(idx)
        }
    }

    fn lower_local_param_decl(
        &mut self,
        localparam_decl: &ast::LocalParameterDeclaration,
    ) -> Option<Idx<DataDecl>> {
        if localparam_decl.token_type().is_some() {
            unimplemented!("Parameter Type");
        } else {
            let src = self.in_file(LocalDataDeclSrc::LocalParamDecl(localparam_decl.to_ptr()));
            let idx = self.next_data_decl_idx();
            let data_decl = DataDecl::ParamDecl(ParamDecl {
                local: true,
                data_type: {
                    let data_type = localparam_decl.data_type_or_implicit()?;
                    Some(self.lower_data_type_or_implicit(&data_type)?)
                },
                sub_decls: self
                    .lower_param_sub_decl_list(&localparam_decl.list_of_param_assignments()?, idx),
            });
            self.arena_data_decl().alloc(data_decl);
            self.src_map_data_decl().insert(src, idx);
            Some(idx)
        }
    }

    fn lower_any_param_decl(
        &mut self,
        any_param_decl: &ast::AnyParameterDeclaration,
    ) -> Option<Idx<DataDecl>> {
        if let Some(param_decl) = any_param_decl.parameter_declaration() {
            self.lower_param_decl(&param_decl)
        } else if let Some(localparam_decl) = any_param_decl.local_parameter_declaration() {
            self.lower_local_param_decl(&localparam_decl)
        } else {
            None
        }
    }

    fn lower_data_decl(&mut self, data_decl: &ast::DataDeclaration) -> Option<Idx<DataDecl>> {
        try_match! {
            data_decl.variable_declaration(), var_decl => {
                self.lower_var_decl(&var_decl)
            },
            data_decl.type_declaration(), _type_decl => {
                unimplemented!("Type Declaration");
            },
            data_decl.net_type_declaration(), _net_type_decl => {
                unimplemented!("Net Type Declaration");
            },
            data_decl.package_import_declaration(), _package_import_decl => {
                unimplemented!("Package Import Declaration");
            },
            _ => None
        }
    }
}
