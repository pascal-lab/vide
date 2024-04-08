use crate::hir_def::{
    data::{self, DataSubDecl, DataType, LowerDataSubDecl, LowerDataType, NetKind},
    expr::{LowerExpr, Select},
    try_match, Ident, InFile, SourceMap,
};
use la_arena::{Arena, Idx, IdxRange, RawIdx};
use syntax::ast::{self, ptr};
use utils::try_;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum PortDirection {
    Input,
    Output,
    Inout,
    Ref,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum PortKind {
    Net(NetKind),
    Var(DataType),
}

// TODO: interface port
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum PortDecl {
    IODecl(IODecl),
    // InterfacePortDecl,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct IODecl {
    pub direction: PortDirection,
    pub port_kind: PortKind,
    pub data_decls: IdxRange<DataSubDecl>,
}

#[derive(Default, Debug, PartialEq, Eq, Clone, Hash)]
pub struct NonAnsiPort {
    pub ident: Option<Ident>,
    pub port_expr: Arena<PortReference>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct PortReference {
    pub ident: Ident,
    pub select: Option<Select>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum AnsiPortDecl {
    IODecl(AnsiIODecl),
    // InterfacePortDecl,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct AnsiIODecl {
    pub direction: PortDirection,
    pub port_kind: PortKind,
    pub sub_decl: Idx<DataSubDecl>,
}

pub(crate) fn lower_port_direction(port_direction: &ast::PortDirection) -> Option<PortDirection> {
    try_match! {
        port_direction.token_input(), _ => Some(PortDirection::Input),
        port_direction.token_output(), _ => Some(PortDirection::Output),
        port_direction.token_inout(), _ => Some(PortDirection::Inout),
        port_direction.token_ref(), _ => Some(PortDirection::Ref),
        _ => None
    }
}

pub(crate) trait LowerPortDecl: LowerDataType + LowerDataSubDecl + LowerExpr {
    fn arena_port_decl(&mut self) -> &mut Arena<PortDecl>;

    fn arena_non_ansi_port(&mut self) -> &mut Arena<NonAnsiPort>;

    fn arena_ansi_port_decl(&mut self) -> &mut Arena<AnsiPortDecl>;

    fn src_map_port_decl(&mut self) -> &mut SourceMap<InFile<ptr::PortDeclarationPtr>, PortDecl>;

    fn src_map_non_ansi_port(&mut self) -> &mut SourceMap<InFile<ptr::PortPtr>, NonAnsiPort>;

    fn src_map_ansi_port_decl(
        &mut self,
    ) -> &mut SourceMap<InFile<ptr::AnsiPortDeclarationPtr>, AnsiPortDecl>;

    fn next_port_decl_idx(&mut self) -> Idx<PortDecl> {
        Idx::from_raw(RawIdx::from(self.arena_port_decl().len() as u32))
    }

    fn next_anis_port_decl_idx(&mut self) -> Idx<AnsiPortDecl> {
        Idx::from_raw(RawIdx::from(self.arena_ansi_port_decl().len() as u32))
    }

    fn lower_port_list(&mut self, port_list: &ast::ListOfPort) {
        for port_node in port_list.ports() {
            try_! {
                let ident = port_node
                    .identifier()
                    .and_then(|ident| self.lower_ident(&ident));
                let port_expr = {
                    let mut arena = Arena::default();
                    for port_ref in port_node.port_expression()?.port_references() {
                        let ident = self.lower_ident(&port_ref.identifier()?)?;
                        let select = self.lower_const_select(&port_ref.constant_select()?)?.traverse();
                        arena.alloc(PortReference { ident, select });
                    }
                    arena.shrink_to_fit();
                    arena
                };
                let src = self.in_file(port_node.to_ptr());
                let idx = self.arena_non_ansi_port().alloc(NonAnsiPort { ident, port_expr });
                self.src_map_non_ansi_port().insert(src, idx);
            };
        }
    }

    // The outer Option is for the error
    // The inner Option is for no port type found
    fn lower_net_port_type(&mut self, net_port_type: &ast::NetPortType) -> Option<PortKind> {
        let net_kind = try_match! {
            net_port_type.data_type_or_implicit(), data_type_or_implicit => {
                let net_type = try_match! {
                    net_port_type.net_type(), net_type => data::lower_net_type(&net_type)?,
                    _ => data::DEFAULT_NET_TYPE,
                };
                let data_type = self.lower_data_type_or_implicit(&data_type_or_implicit)?;
                NetKind::Default { net_type, data_type }
            },
            net_port_type.identifier(), _ident => {
                unimplemented!("net_port_type ::= net_type_identifier");
            },
            net_port_type.token_interconnect(), _ => {
                unimplemented!("net_port_type ::= interconnect implicit_data_type");
            },
            _ => return None,
        };
        Some(PortKind::Net(net_kind))
    }

    fn lower_var_port_type(&mut self, var_port_type: &ast::VariablePortType) -> Option<PortKind> {
        let var_data_type = var_port_type.var_data_type()?;
        let data_type = try_match! {
            var_data_type.data_type(), data_type => {
                self.lower_data_type(&data_type)?
            },
            var_data_type.data_type_or_implicit(), data_type_or_implicit => {
                self.lower_data_type_or_implicit(&data_type_or_implicit)?
            },
            _ => {return None;}
        };
        Some(PortKind::Var(data_type))
    }

    fn lower_port_decl(&mut self, port_decl_node: &ast::PortDeclaration) -> Option<Idx<PortDecl>> {
        try_match! {
            port_decl_node.inout_declaration(), inout_decl => {
                let src = self.in_file(port_decl_node.to_ptr());
                let idx = self.next_port_decl_idx();
                let direction = PortDirection::Inout;
                let port_kind = self.lower_net_port_type(&inout_decl.net_port_type()?)?;
                let data_decls = self.lower_port_ident_list(&inout_decl.list_of_port_identifiers()?, idx);
                self.arena_port_decl().alloc(PortDecl::IODecl(IODecl {
                    direction, port_kind, data_decls,
                }));
                self.src_map_port_decl().insert(src, idx);
                Some(idx)
            },
            port_decl_node.input_declaration(), input_decl => {
                let src = self.in_file(port_decl_node.to_ptr());
                let idx = self.next_port_decl_idx();
                let direction = PortDirection::Input;
                let (port_kind, data_decls) = try_match! {
                    input_decl.net_port_type(), net_port_type => (
                        self.lower_net_port_type(&net_port_type)?,
                        self.lower_port_ident_list(&input_decl.list_of_port_identifiers()?, idx)
                    ),
                    input_decl.variable_port_type(), var_port_type => (
                        self.lower_var_port_type(&var_port_type)?,
                        self.lower_var_ident_list(&input_decl.list_of_variable_identifiers()?, idx)
                    ),
                    _ => {return None;}
                };
                self.arena_port_decl().alloc(PortDecl::IODecl(IODecl {
                    direction, port_kind, data_decls,
                }));
                self.src_map_port_decl().insert(src, idx);
                Some(idx)
            },
            port_decl_node.output_declaration(), output_decl => {
                let src = self.in_file(port_decl_node.to_ptr());
                let idx = self.next_port_decl_idx();
                let direction = PortDirection::Output;
                let (port_kind, data_decls) = try_match! {
                    output_decl.net_port_type(), net_port_type => (
                        self.lower_net_port_type(&net_port_type)?,
                        self.lower_port_ident_list(&output_decl.list_of_port_identifiers()?, idx)
                    ),
                    output_decl.variable_port_type(), var_port_type => (
                        self.lower_var_port_type(&var_port_type)?,
                        self.lower_var_port_ident_list(&output_decl.list_of_variable_port_identifiers()?, idx)
                    ),
                    _ => {return None;}
                };
                self.arena_port_decl().alloc(PortDecl::IODecl(IODecl {
                    direction, port_kind, data_decls,
                }));
                self.src_map_port_decl().insert(src, idx);
                Some(idx)
            },
            port_decl_node.ref_declaration(), ref_decl => {
                let src = self.in_file(port_decl_node.to_ptr());
                let idx = self.next_port_decl_idx();
                let direction = PortDirection::Ref;
                let port_kind = self.lower_var_port_type(&ref_decl.variable_port_type()?)?;
                let data_decls = self.lower_var_ident_list(&ref_decl.list_of_variable_identifiers()?, idx);
                self.arena_port_decl().alloc(PortDecl::IODecl(IODecl {
                    direction, port_kind, data_decls,
                }));
                self.src_map_port_decl().insert(src, idx);
                Some(idx)
            },
            _ => {
                unimplemented!("port_declaration ::= interface_port_declaration")
            }
        }
    }

    fn lower_ansi_port_decl_list(&mut self, port_decl_list: &ast::ListOfPortDeclaration) {
        let mut direction = PortDirection::Inout;
        let mut port_kind = PortKind::Net(NetKind::Default {
            net_type: data::DEFAULT_NET_TYPE,
            data_type: data::DataType::IntegerType(data::IntegerType::Logic {
                dimensions: None,
                sign: false,
            }),
        });

        for port_decl in port_decl_list.ansi_port_declarations() {
            let src = self.in_file(port_decl.to_ptr());
            let idx = self.next_anis_port_decl_idx();
            try_! {
                try_match!{
                    port_decl.net_port_header(), net_port_header => {
                        if let Some(dir) = net_port_header.port_direction() {
                            direction = lower_port_direction(&dir)?;
                        };
                        port_kind = self.lower_net_port_type(&net_port_header.net_port_type()?)?;
                        let sub_decl = self.lower_ansi_port_decl(&port_decl, idx)?;
                        self.arena_ansi_port_decl().alloc(
                            AnsiPortDecl::IODecl(AnsiIODecl {
                                direction, port_kind: port_kind.clone(), sub_decl,
                            })
                        );
                        self.src_map_ansi_port_decl().insert(src, idx);
                    },
                    port_decl.variable_port_header(), var_port_header => {
                        try_match!{
                            var_port_header.port_direction(), direction_node => {
                                direction = lower_port_direction(&direction_node)?;
                            }
                        };
                        port_kind = self.lower_var_port_type(&var_port_header.variable_port_type()?)?;
                        let sub_decl = self.lower_ansi_port_decl(&port_decl, idx)?;
                        self.arena_ansi_port_decl().alloc(
                            AnsiPortDecl::IODecl(AnsiIODecl {
                                direction, port_kind: port_kind.clone(), sub_decl,
                            })
                        );
                        self.src_map_ansi_port_decl().insert(src, idx);
                    },
                    port_decl.interface_port_header(), _interface_port_header => {
                        unimplemented!("ansi_port_declaration ::= interface_port_header port_identifier {{unpacked_dimension}}
                        [=constant_expression]")
                    },
                    port_decl.token_dot(), _ => {
                        unimplemented!("ansi_port_declaration ::= [port_direction].port_identifier([expression])")
                    },
                    _ => {
                        let sub_decl = self.lower_ansi_port_decl(&port_decl, idx)?;
                        self.arena_ansi_port_decl().alloc(
                            AnsiPortDecl::IODecl(AnsiIODecl {
                                direction, port_kind: port_kind.clone(), sub_decl,
                            })
                        );
                        self.src_map_ansi_port_decl().insert(src, idx);
                    }
                };
            };
        }
    }
}
