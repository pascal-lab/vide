use crate::{
    container::InFile,
    hir_def::{
        data::{
            self, DataType, IntegerType, LowerDataType, LowerSubDecl, NetKind, SubDecl,
            DEFAULT_NET_TYPE,
        },
        expr::{LowerExpr, Select},
        try_match, Ident, SourceMap,
    },
};
use itertools::Either;
use la_arena::{Arena, Idx, IdxRange, RawIdx};
use smallvec::{smallvec, SmallVec};
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
    IOPortDef(IOPortDef),
    // InterfacePortDecl,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct IOPortDef {
    pub direction: PortDirection,
    pub kind: PortKind,
    pub sub_decls: IdxRange<SubDecl>,
}

#[derive(Default, Debug, PartialEq, Eq, Clone, Hash)]
pub struct Port {
    pub label: Option<Ident>,
    pub expr: SmallVec<[PortReference; 1]>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct PortReference {
    pub ident: Ident,
    pub select: Option<Select>,
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

pub(crate) trait LowerPortDecl: LowerDataType + LowerSubDecl + LowerExpr {
    fn arena_port_def(&mut self) -> &mut Arena<PortDecl>;

    fn arena_port_decl(&mut self) -> &mut Arena<Port>;

    fn src_map_port_def(
        &mut self,
    ) -> &mut SourceMap<
        Either<InFile<ptr::PortDeclarationPtr>, InFile<ptr::AnsiPortDeclarationPtr>>,
        PortDecl,
    >;

    fn src_map_port_decl(
        &mut self,
    ) -> &mut SourceMap<Either<InFile<ptr::PortPtr>, InFile<ptr::AnsiPortDeclarationPtr>>, Port>;

    fn next_port_def_idx(&mut self) -> Idx<PortDecl> {
        Idx::from_raw(RawIdx::from(self.arena_port_def().len() as u32))
    }

    fn lower_port_list(&mut self, port_list: &ast::ListOfPort) {
        for port in port_list.ports() {
            try_! {
                let label = port.identifier().and_then(|ident| self.lower_ident(&ident));
                let expr = port
                    .port_expression()?
                    .port_references()
                    .filter_map(|port_ref| {
                        let ident = self.lower_ident(&port_ref.identifier()?)?;
                        let select = try_match! {
                            port_ref.constant_select(), select => {
                                self.lower_const_select(&select)?.traverse()
                            },
                            _ => None,
                        };
                        Some(PortReference { ident, select })
                    })
                    .collect();
                let src = self.in_file(port.to_ptr());
                let idx = self.arena_port_decl().alloc(Port { label, expr });
                self.src_map_port_decl().insert(Either::Left(src), idx);
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
                let data_type = self.lower_data_type_or_implicit(&Some(data_type_or_implicit))?;
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
            var_data_type.token_var(), _ => {
                let data_type_or_implicit = var_data_type.data_type_or_implicit();
                self.lower_data_type_or_implicit(&data_type_or_implicit)?
            },
            _ => {return None;}
        };
        Some(PortKind::Var(data_type))
    }

    fn lower_port_decl(&mut self, port_decl_node: &ast::PortDeclaration) -> Option<Idx<PortDecl>> {
        let src = self.in_file(port_decl_node.to_ptr());
        let port_def_idx = self.next_port_def_idx();
        let (direction, port_kind, data_decls) = try_match! {
            port_decl_node.inout_declaration(), inout_decl => {
                let port_kind = self.lower_net_port_type(&inout_decl.net_port_type()?)?;
                let data_decls = self.lower_net_ident_list(&inout_decl.list_of_port_identifiers()?, port_def_idx);
                (PortDirection::Inout, port_kind, data_decls)
            },
            port_decl_node.input_declaration(), input_decl => {
                let (port_kind, data_decls) = try_match! {
                    input_decl.net_port_type(), net_port_type => (
                        self.lower_net_port_type(&net_port_type)?,
                        self.lower_net_ident_list(&input_decl.list_of_port_identifiers()?, port_def_idx)
                    ),
                    input_decl.variable_port_type(), var_port_type => (
                        self.lower_var_port_type(&var_port_type)?,
                        self.lower_var_ident_list(&input_decl.list_of_variable_identifiers()?, port_def_idx)
                    ),
                    _ => {
                        // TODO: fix
                        let port_kind = PortKind::Net(NetKind::Default {
                            net_type: DEFAULT_NET_TYPE,
                            data_type: DataType::Int(IntegerType::Logic {
                                dimensions: None,
                                sign: false,
                            })
                        });
                        let data_decls = try_match! {
                            input_decl.list_of_port_identifiers(), list => {
                                self.lower_net_ident_list(&list, port_def_idx)
                            },
                            input_decl.list_of_variable_identifiers(), list => {
                                self.lower_var_ident_list(&list, port_def_idx)
                            },
                            _ => {return None;}
                        };
                        (port_kind, data_decls)
                    }
                };
                (PortDirection::Input, port_kind, data_decls)
            },
            port_decl_node.output_declaration(), output_decl => {
                let (port_kind, data_decls) = try_match! {
                    output_decl.net_port_type(), net_port_type => (
                        self.lower_net_port_type(&net_port_type)?,
                        self.lower_net_ident_list(&output_decl.list_of_port_identifiers()?, port_def_idx)
                    ),
                    output_decl.variable_port_type(), var_port_type => (
                        self.lower_var_port_type(&var_port_type)?,
                        self.lower_var_port_ident_list(&output_decl.list_of_variable_port_identifiers()?, port_def_idx)
                    ),
                    _ => {
                        // TODO: fix
                        let port_kind = PortKind::Net(NetKind::Default {
                            net_type: DEFAULT_NET_TYPE,
                            data_type: DataType::Int(IntegerType::Logic {
                                dimensions: None,
                                sign: false,
                            })
                        });
                        let data_decls = try_match! {
                            output_decl.list_of_port_identifiers(), list => {
                                self.lower_net_ident_list(&list, port_def_idx)
                            },
                            output_decl.list_of_variable_port_identifiers(), list => {
                                self.lower_var_port_ident_list(&list, port_def_idx)
                            },
                            _ => {return None;}
                        };
                        (port_kind, data_decls)
                    }
                };
                (PortDirection::Output, port_kind, data_decls)
            },
            port_decl_node.ref_declaration(), ref_decl => {
                let port_kind = self.lower_var_port_type(&ref_decl.variable_port_type()?)?;
                let data_decls = self.lower_var_ident_list(&ref_decl.list_of_variable_identifiers()?, port_def_idx);
                (PortDirection::Ref, port_kind, data_decls)
            },
            _ => {
                unimplemented!("port_declaration ::= interface_port_declaration")
            }
        };

        self.arena_port_def().alloc(PortDecl::IOPortDef(IOPortDef {
            direction,
            kind: port_kind,
            sub_decls: data_decls,
        }));
        self.src_map_port_def().insert(Either::Left(src), port_def_idx);
        Some(port_def_idx)
    }

    fn lower_ansi_port_decl_list(&mut self, port_decl_list: &ast::ListOfPortDeclaration) {
        let mut port_decls = port_decl_list.ansi_port_declarations().peekable();
        while port_decls.peek().is_some() {
            if try_! {
                let mut port_decl = port_decls.next().unwrap();
                let src = self.in_file(port_decl.to_ptr());

                let (direction, port_kind) = try_match! {
                    port_decl.net_port_header(), net_port_header => {
                        let direction = try_match! {
                            net_port_header.port_direction(), direction_node => lower_port_direction(&direction_node)?,
                            _ => PortDirection::Inout
                        };
                        let port_kind = try_match!{
                            net_port_header.net_port_type(), net_port_type => {
                                self.lower_net_port_type(&net_port_type)?
                            },
                            _ => PortKind::Net(NetKind::Default {
                                net_type: data::DEFAULT_NET_TYPE,
                                data_type: DataType::Int(IntegerType::Logic {
                                    dimensions: None,
                                    sign: false
                                })
                            })
                        };
                        (direction, port_kind)
                    },
                    port_decl.variable_port_header(), var_port_header => {
                        let direction = try_match! {
                            var_port_header.port_direction(), direction_node => lower_port_direction(&direction_node)?,
                            _ => PortDirection::Inout
                        };
                        let port_kind = self.lower_var_port_type(&var_port_header.variable_port_type()?)?;
                        (direction, port_kind)
                    },
                    port_decl.interface_port_header(), _interface_port_header => {
                        unimplemented!("ansi_port_declaration ::= interface_port_header port_identifier {{unpacked_dimension}}
                    [=constant_expression]")
                    },
                    port_decl.token_dot(), _ => {
                        unimplemented!("ansi_port_declaration ::= [port_direction].port_identifier([expression])")
                    },
                    _ => unreachable!(),
                };

                let sub_decl_begin_idx = self.next_sub_decl_idx();
                let port_decl_idx = self.next_port_def_idx();

                loop {
                    let sub_decl_idx = self.lower_ansi_port_decl(&port_decl, port_decl_idx)?;
                    let expr = smallvec![PortReference {
                        ident: self.arena_sub_decl()[sub_decl_idx].ident.clone(),
                        select: None,
                    }];
                    let src = self.in_file(port_decl.to_ptr());
                    let port_decl_idx =
                        self.arena_port_decl().alloc(Port { label: None, expr });
                    self.src_map_port_decl().insert(Either::Right(src.clone()), port_decl_idx);

                    port_decl = match port_decls.next_if(|port_decl| {
                        port_decl.net_port_header().is_none() && port_decl.variable_port_header().is_none()
                    }) {
                        Some(port_decl) => port_decl,
                        None => break,
                    };
                }

                let sub_decls = IdxRange::new(sub_decl_begin_idx..self.next_sub_decl_idx());
                self.arena_port_def().alloc(PortDecl::IOPortDef(IOPortDef {
                    direction,
                    kind: port_kind,
                    sub_decls,
                }));

                self.src_map_port_def().insert(Either::Right(src), port_decl_idx);
                Some(())
            }.is_none() {
                loop {
                    if port_decls
                        .next_if(|port_decl| {
                            port_decl.net_port_header().is_some()
                                || port_decl.variable_port_header().is_some()
                        })
                        .is_none()
                    {
                        break;
                    }
                }
                continue;
            }
        }
    }
}
