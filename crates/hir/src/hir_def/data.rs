use crate::hir_def::{
    expr::{LocalExprSrcId, LowerExprSrc},
    try_match, Ident,
};
use la_arena::{Arena, ArenaMap, Idx, IdxRange, RawIdx};
use smallvec::SmallVec;
use smol_str::SmolStr;
use syntax::ast::{self, ptr, AstNode};
use utils::try_;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum DataType {
    Implicit{dimensions: Option<SmallVec<[Dimension; 1]>>, sign: bool},
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
    Byte{sign: bool},
    ShortInt{sign: bool},
    Int{sign: bool},
    LongInt{sign: bool},
    Integer{sign: bool},
    Time{sign: bool},
    Bit{dimensions: Option<SmallVec<[Dimension; 1]>>, sign: bool},
    Logic{dimensions: Option<SmallVec<[Dimension; 1]>>, sign: bool},
    Reg{dimensions: Option<SmallVec<[Dimension; 1]>>, sign: bool},
}

pub(crate) fn lower_signing(signing: &ast::Signing) -> Option<bool> {
    try_match!{
        signing.token_signed(), _ => Some(true),
        signing.token_unsigned(), _ => Some(false),
        _ => None,
    }
}

pub(crate) trait LowerDataType: LowerDimension {
    fn lower_data_type(&mut self, data_type: &ast::DataType) -> Option<DataType> {
        try_match!{
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
        try_match!{
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
    Range(LocalExprSrcId, LocalExprSrcId),
    Expr(LocalExprSrcId),
    // Unsized,
}

pub(crate) trait LowerDimension: LowerExprSrc {
    fn lower_packed_dimension(
        &mut self,
        packed_dimension: &ast::PackedDimension,
    ) -> Option<Dimension> {
        try_match! {
            packed_dimension.constant_range(), range => {
                let left_expr_node = range.constant_expressions().next()?;
                let right_expr_node = range.constant_expressions().next()?;
                Some(Dimension::Range(
                    self.lower_const_expr_src(&left_expr_node),
                    self.lower_const_expr_src(&right_expr_node),
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
                    self.lower_const_expr_src(&left_expr_node),
                    self.lower_const_expr_src(&right_expr_node),
                ))
            },
            unpacked_dimension.constant_expression(), expr => {
                Some(Dimension::Expr(self.lower_const_expr_src(&expr)))
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
    Default{net_type: NetType, data_type: DataType},
    // TODO: net_type_identifier
    // Ident{ident: Ident},
}

pub const DEFAULT_NET_TYPE: NetType = NetType::Wire;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct DataSubDecl {
    pub ident: Ident,
    pub dimensions: Option<SmallVec<[Dimension; 1]>>,
    pub expr: Option<LocalExprSrcId>,
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
}

pub(crate) trait LowerDataSubDecl: LowerDimension + LowerExprSrc {
    fn arena_data_sub_decls(&mut self) -> &mut Arena<DataSubDecl>;

    fn src_map_data_sub_decls(&mut self) -> &mut ArenaMap<Idx<DataSubDecl>, LocalDataSubDeclSrc>;

    fn next_data_sub_decl_idx(&mut self) -> Idx<DataSubDecl> {
        Idx::from_raw(RawIdx::from(self.arena_data_sub_decls().len() as u32))
    }

    fn lower_net_sub_decl(
        &mut self,
        net_assign: &ast::NetDeclAssignment,
    ) -> Option<Idx<DataSubDecl>> {
        let ident: SmolStr = net_assign.identifier()?.to_text(self.file_text())?.into();
        let expr = net_assign.expression().map(|expr| self.lower_expr_src(&expr));
        let mut dimensions = SmallVec::<[Dimension; 1]>::new();
        for unpacked_dimension in net_assign.unpacked_dimensions() {
            dimensions.push(self.lower_unpacked_dimension(&unpacked_dimension)?);
        }
        let dimensions = if dimensions.is_empty() { None } else { Some(dimensions) };
        let idx = self.arena_data_sub_decls().alloc(DataSubDecl { ident, dimensions, expr });
        self.src_map_data_sub_decls()
            .insert(idx, LocalDataSubDeclSrc::NetDeclAssign(net_assign.to_ptr()));
        Some(idx)
    }

    fn lower_net_sub_decl_list(
        &mut self,
        net_decl_list: &ast::ListOfNetDeclAssignment,
    ) -> IdxRange<DataSubDecl> {
        let begin_idx = self.next_data_sub_decl_idx();
        for net_decl in net_decl_list.net_decl_assignments() {
            self.lower_net_sub_decl(&net_decl);
        }
        let end_idx = self.next_data_sub_decl_idx();
        IdxRange::new(begin_idx..end_idx)
    }

    fn lower_var_sub_decl(
        &mut self,
        var_assign: &ast::VariableDeclAssignment,
    ) -> Option<Idx<DataSubDecl>> {
        let ident: SmolStr = var_assign.identifier()?.to_text(self.file_text())?.into();
        let expr = var_assign.expression().map(|expr| self.lower_expr_src(&expr));
        let mut dimensions = SmallVec::<[Dimension; 1]>::new();
        for var_dimension in var_assign.variable_dimensions() {
            dimensions.push(self.lower_var_dimension(&var_dimension)?);
        }
        let dimensions = if dimensions.is_empty() { None } else { Some(dimensions) };
        let idx = self.arena_data_sub_decls().alloc(DataSubDecl { ident, dimensions, expr });
        self.src_map_data_sub_decls()
            .insert(idx, LocalDataSubDeclSrc::VarDeclAssign(var_assign.to_ptr()));
        Some(idx)
    }

    fn lower_var_sub_decl_list(
        &mut self,
        var_decl_list: &ast::ListOfVariableDeclAssignment,
    ) -> IdxRange<DataSubDecl> {
        let begin_idx = self.next_data_sub_decl_idx();
        for var_decl in var_decl_list.variable_decl_assignments() {
            self.lower_var_sub_decl(&var_decl);
        }
        let end_idx = self.next_data_sub_decl_idx();
        IdxRange::new(begin_idx..end_idx)
    }

    fn lower_param_sub_decl(
        &mut self,
        param_assign: &ast::ParamAssignment,
    ) -> Option<Idx<DataSubDecl>> {
        let ident: SmolStr = param_assign.identifier()?.to_text(self.file_text())?.into();
        let expr = param_assign
            .constant_param_expression()
            .map(|expr| self.lower_const_param_expr_src(&expr));
        let mut dimensions = SmallVec::<[Dimension; 1]>::new();
        for unpacked_dimension in param_assign.unpacked_dimensions() {
            dimensions.push(self.lower_unpacked_dimension(&unpacked_dimension)?);
        }
        let dimensions = if dimensions.is_empty() { None } else { Some(dimensions) };
        let idx = self.arena_data_sub_decls().alloc(DataSubDecl { ident, dimensions, expr });
        self.src_map_data_sub_decls()
            .insert(idx, LocalDataSubDeclSrc::ParamAssign(param_assign.to_ptr()));
        Some(idx)
    }

    fn lower_param_sub_decl_list(
        &mut self,
        param_decl_list: &ast::ListOfParamAssignment,
    ) -> IdxRange<DataSubDecl> {
        let begin_idx = self.next_data_sub_decl_idx();
        for param_decl in param_decl_list.param_assignments() {
            self.lower_param_sub_decl(&param_decl);
        }
        let end_idx = self.next_data_sub_decl_idx();
        IdxRange::new(begin_idx..end_idx)
    }

    fn lower_ansi_port_decl(
        &mut self,
        ansi_port_decl: &ast::AnsiPortDeclaration,
    ) -> Option<Idx<DataSubDecl>> {
        let ident: SmolStr = ansi_port_decl.identifier()?.to_text(self.file_text())?.into();
        let expr = ansi_port_decl.expression().map(|expr| self.lower_expr_src(&expr));
        let mut dimensions = SmallVec::<[Dimension; 1]>::new();
        for unpacked_dimension in ansi_port_decl.unpacked_dimensions() {
            dimensions.push(self.lower_unpacked_dimension(&unpacked_dimension)?);
        }
        for var_dimension in ansi_port_decl.variable_dimensions() {
            dimensions.push(self.lower_var_dimension(&var_dimension)?);
        }
        let dimensions = if dimensions.is_empty() { None } else { Some(dimensions) };
        let idx = self.arena_data_sub_decls().alloc(DataSubDecl { ident, dimensions, expr });
        self.src_map_data_sub_decls()
            .insert(idx, LocalDataSubDeclSrc::AnsiPortDecl(ansi_port_decl.to_ptr()));
        Some(idx)
    }
}

// Todo: [ drive_strength | charge_strength ] [ vectored | scalared ]  [ delay3 ]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct NetDecl {
    pub net_kind: NetKind,
    // TODO: [ vectored | scalared ]
    // pub vectored: bool,
    // pub scalared: bool,
    // TODO: drive_strength, charge_strength, delay3
    pub sub_decls: IdxRange<DataSubDecl>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct VarDecl {
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

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum LocalParamPortDeclSrc {
    ParamAssignList(ptr::ListOfParamAssignmentPtr),
    ParamPortDecl(ptr::ParameterPortDeclarationPtr),
}

pub(crate) trait LowerParamDecl: LowerDataType + LowerDataSubDecl {
    fn lower_param_decl(&mut self, param_decl: &ast::ParameterDeclaration) -> Option<ParamDecl> {
        if param_decl.token_type().is_some() {
            unimplemented!("Parameter Type");
        } else {
            try_!(ParamDecl {
                local: false,
                data_type: {
                    let data_type = param_decl.data_type_or_implicit()?;
                    Some(self.lower_data_type_or_implicit(&data_type)?)
                },
                sub_decls: self.lower_param_sub_decl_list(&param_decl.list_of_param_assignments()?),
            })
        }
    }

    fn lower_local_param_decl(
        &mut self,
        localparam_decl: &ast::LocalParameterDeclaration,
    ) -> Option<ParamDecl> {
        if localparam_decl.token_type().is_some() {
            unimplemented!("Parameter Type");
        } else {
            Some(ParamDecl {
                local: true,
                data_type: {
                    let data_type = localparam_decl.data_type_or_implicit()?;
                    Some(self.lower_data_type_or_implicit(&data_type)?)
                },
                sub_decls: self
                    .lower_param_sub_decl_list(&localparam_decl.list_of_param_assignments()?),
            })
        }
    }

    fn lower_any_param_decl(
        &mut self,
        any_param_decl: &ast::AnyParameterDeclaration,
    ) -> Option<ParamDecl> {
        if let Some(param_decl) = any_param_decl.parameter_declaration() {
            self.lower_param_decl(&param_decl)
        } else if let Some(localparam_decl) = any_param_decl.local_parameter_declaration() {
            self.lower_local_param_decl(&localparam_decl)
        } else {
            None
        }
    }
}

// 23.3.2 Module instantiation syntax
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct OrderedPortAssignment {
    expr: LocalExprSrcId,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct NamedPortAssignment {
    ident: Ident,
    expr: Option<LocalExprSrcId>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum PortAssignmentsList {
    Ordered(Box<[OrderedPortAssignment]>),
    Named(Box<[NamedPortAssignment]>),
}

// TODO: TypeDecl, NetTypeDecl
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum DataDecl {
    NetDecl(NetDecl),
    VarDecl(VarDecl),
    ParamDecl(ParamDecl),
}
