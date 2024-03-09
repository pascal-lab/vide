use crate::hir_def::{
    expr::{LocalExprSrcId, LowerExprSrc},
    Ident,
};
use la_arena::{Arena, ArenaMap, Idx, IdxRange, RawIdx};
use smallvec::SmallVec;
use smol_str::SmolStr;
use syntax::ast::{self, ptr, AstNode};
use utils::try_;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum DataType {
    IntegerType,
    NonIntegerType,
    StructUnion,
    Enum,
    String,
    Implicit,
    // TODO: for paramdecl syntax:
    //      parameter_declaration ::= parameter type list_of_type_assignments
    // TODO: complete all the data types
}

pub(crate) trait LowerDataType: LowerExprSrc {
    fn lower_data_type(&mut self, data_type: &ast::DataType) -> DataType {
        unimplemented!("Lower data_type")
    }

    fn lower_data_type_or_implicit(
        &mut self,
        data_type_or_mplicit: &ast::DataTypeOrImplicit,
    ) -> DataType {
        if let Some(data_type) = data_type_or_mplicit.data_type() {
            self.lower_data_type(&data_type)
        } else {
            unimplemented!("Lower implicit data_type")
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
    // Those SubDecls Below is edited for convenience
    PortIdentDecl(ptr::PortIdentifierDeclarationPtr),
    VarIdentDecl(ptr::VariableIdentifierDeclarationPtr),
}

pub(crate) trait LowerDataSubDecl: LowerExprSrc {
    fn arena_data_sub_decls(&mut self) -> &mut Arena<DataSubDecl>;

    fn src_map_data_sub_decls(&mut self) -> &mut ArenaMap<Idx<DataSubDecl>, LocalDataSubDeclSrc>;

    fn next_data_sub_decl_idx(&mut self) -> Idx<DataSubDecl> {
        Idx::from_raw(RawIdx::from(self.arena_data_sub_decls().len() as u32))
    }

    fn lower_unpacked_dimension(
        &mut self,
        unpacked_dimension: &ast::UnpackedDimension,
    ) -> Option<Dimension> {
        if let Some(range) = unpacked_dimension.constant_range() {
            let left_expr_node = range.constant_expressions().next()?;
            let right_expr_node = range.constant_expressions().next()?;
            Some(Dimension::Range(
                self.lower_const_expr_src(&left_expr_node),
                self.lower_const_expr_src(&right_expr_node),
            ))
        } else if let Some(expr) = unpacked_dimension.constant_expression() {
            Some(Dimension::Expr(self.lower_const_expr_src(&expr)))
        } else {
            None
        }
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
            if let Some(unpacked) = var_dimension.unpacked_dimension() {
                dimensions.push(self.lower_unpacked_dimension(&unpacked)?);
            } else if let Some(_associative) = var_dimension.associative_dimension() {
                unimplemented!("Associative Dimension");
            } else if let Some(_queue) = var_dimension.queue_dimension() {
                unimplemented!("Queue Dimension");
            } else if let Some(_unsized_dimension) = var_dimension.unsized_dimension() {
                unimplemented!("Unsized Dimension");
            } else {
                return None;
            }
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
}

// Todo: [ drive_strength | charge_strength ] [ vectored | scalared ]  [ delay3 ]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct NetDecl {
    pub net_type: NetType,
    // TODO: [ vectored | scalared ]
    // pub vectored: bool,
    // pub scalared: bool,
    pub signed: bool,
    // TODO: drive_strength, charge_strength, delay3
    pub data_type: DataType,
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

pub(crate) trait LowerParamDecl: LowerDataType + LowerDataSubDecl {
    fn lower_param_decl(&mut self, param_decl: &ast::ParameterDeclaration) -> Option<ParamDecl> {
        if param_decl.token_type().is_some() {
            unimplemented!("Parameter Type");
        } else {
            Some(ParamDecl {
                local: false,
                data_type: param_decl.data_type_or_implicit().map(|data_type_or_implicit| {
                    self.lower_data_type_or_implicit(&data_type_or_implicit)
                }),
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
                data_type: localparam_decl.data_type_or_implicit().map(|data_type_or_implicit| {
                    self.lower_data_type_or_implicit(&data_type_or_implicit)
                }),
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
