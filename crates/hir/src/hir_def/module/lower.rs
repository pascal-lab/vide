use crate::hir_def::{
    data::{
        DataDecl, DataDeclSrc, DataSubDecl, DataSubDeclSrc, LocalDataDeclSrc, LowerDataDecl,
        LowerDataSubDecl, LowerDataType, LowerDimension, ParamDecl,
    },
    expr::{Expr, ExprSrc, LowerExpr},
    literal::LowerLiteral,
    lower::Lower,
    module::{
        port::{AnsiPortDecl, LowerPortDecl, NonAnsiPort, PortDecl},
        ModuleDecl, ModuleSourceMap,
    },
    try_match, HirFileId, InFile, SourceMap,
};
use la_arena::{Arena, IdxRange};
use syntax::ast::{self, ptr};
use utils::try_;

pub(crate) struct ModuleLowerCtx<'a> {
    pub hir_file_id: HirFileId,
    pub module_decl: &'a mut ModuleDecl,
    pub module_src_map: &'a mut ModuleSourceMap,
    pub file_text: &'a str,
}

impl<'a> ModuleLowerCtx<'a> {
    pub(crate) fn lower_module_decl(&mut self, module_node: &ast::ModuleDeclaration) {
        // TODO: package_import_declaration

        try_match! {
            module_node.module_ansi_header(), ansi_header => {
                if let Some(param_port_list) = ansi_header.parameter_port_list() {
                    self.lower_param_port_list(&param_port_list);
                }
                if let Some(port_decl_list) = ansi_header.list_of_port_declarations() {
                    self.lower_ansi_port_decl_list(&port_decl_list);
                }
                for non_port_module_item in module_node.non_port_module_items() {
                    self.lower_non_port_module_item(&non_port_module_item);
                }
            },
            module_node.module_nonansi_header(), non_ansi_header => {
                if let Some(param_port_list) = non_ansi_header.parameter_port_list() {
                    self.lower_param_port_list(&param_port_list);
                }
                try_!(self.lower_port_list(&non_ansi_header.list_of_ports()?));
                for module_item in module_node.module_items() {
                    self.lower_module_item(&module_item);
                }
            },
            _ => {}
        };
    }

    fn lower_param_port_list(&mut self, param_port_list: &ast::ParameterPortList) {
        let begin_idx = self.next_data_decl_idx();
        if let Some(param_assign_list) = param_port_list.list_of_param_assignments() {
            let idx = self.next_data_decl_idx();
            let sub_decls = self.lower_param_sub_decl_list(&param_assign_list, idx);
            let src = self.in_file(LocalDataDeclSrc::ParamAssignList(param_assign_list.to_ptr()));
            self.arena_data_decl().alloc(DataDecl::ParamDecl(ParamDecl {
                local: false,
                data_type: None,
                sub_decls,
            }));
            self.src_map_data_decl().insert(src, idx);
        }
        for param_port_decl in param_port_list.parameter_port_declarations() {
            try_! {
                try_match!{
                    param_port_decl.any_parameter_declaration(), any_param_decl => {
                        self.lower_any_param_decl(&any_param_decl);
                    },
                    param_port_decl.data_type(), data_type => {
                        let src = self.in_file(LocalDataDeclSrc::ParamPortDecl(param_port_decl.to_ptr()));
                        let idx = self.next_data_decl_idx();
                        let data_type = self.lower_data_type(&data_type)?;
                        let sub_decls = self.lower_param_sub_decl_list(&param_port_decl.list_of_param_assignments()?, idx);
                        self.arena_data_decl().alloc(DataDecl::ParamDecl(ParamDecl {
                            local: false,
                            data_type: Some(data_type),
                            sub_decls,
                        }));
                        self.src_map_data_decl().insert(src, idx);
                    },
                    param_port_decl.token_type(), _token_type => {
                        unimplemented!("parameter_port_declaration ::= type list_of_type_assignments");
                    },
                    _ => { return None; }
                };
            };
        }
        let end_idx = self.next_data_decl_idx();
        self.module_decl.param_port_list = Some(IdxRange::new(begin_idx..end_idx));
    }
}

impl Lower for ModuleLowerCtx<'_> {
    fn file_id(&self) -> HirFileId {
        self.hir_file_id
    }

    fn file_text(&self) -> &str {
        self.file_text
    }
}

impl LowerLiteral for ModuleLowerCtx<'_> {}

impl LowerExpr for ModuleLowerCtx<'_> {
    fn arena_expr(&mut self) -> &mut Arena<Expr> {
        &mut self.module_decl.data.exprs
    }

    fn src_map_expr(&mut self) -> &mut SourceMap<ExprSrc, Expr> {
        &mut self.module_src_map.expr
    }
}

impl LowerDataType for ModuleLowerCtx<'_> {}

impl LowerDimension for ModuleLowerCtx<'_> {}

impl LowerDataSubDecl for ModuleLowerCtx<'_> {
    fn arena_data_sub_decl(&mut self) -> &mut Arena<DataSubDecl> {
        &mut self.module_decl.data.data_sub_decls
    }

    fn src_map_data_sub_decl(&mut self) -> &mut SourceMap<DataSubDeclSrc, DataSubDecl> {
        &mut self.module_src_map.data_sub_decl
    }
}

impl LowerDataDecl for ModuleLowerCtx<'_> {
    fn arena_data_decl(&mut self) -> &mut Arena<DataDecl> {
        &mut self.module_decl.data.data_decls
    }

    fn src_map_data_decl(&mut self) -> &mut SourceMap<DataDeclSrc, DataDecl> {
        &mut self.module_src_map.data_decl
    }
}

impl LowerPortDecl for ModuleLowerCtx<'_> {
    fn arena_port_decl(&mut self) -> &mut Arena<PortDecl> {
        &mut self.module_decl.data.port_decls
    }

    fn arena_non_ansi_port(&mut self) -> &mut Arena<NonAnsiPort> {
        &mut self.module_decl.non_ansi_ports
    }

    fn arena_ansi_port_decl(&mut self) -> &mut Arena<AnsiPortDecl> {
        &mut self.module_decl.ansi_port_decls
    }

    fn src_map_port_decl(&mut self) -> &mut SourceMap<InFile<ptr::PortDeclarationPtr>, PortDecl> {
        &mut self.module_src_map.port_decl
    }

    fn src_map_non_ansi_port(&mut self) -> &mut SourceMap<InFile<ptr::PortPtr>, NonAnsiPort> {
        &mut self.module_src_map.non_ansi_port
    }

    fn src_map_ansi_port_decl(
        &mut self,
    ) -> &mut SourceMap<InFile<ptr::AnsiPortDeclarationPtr>, AnsiPortDecl> {
        &mut self.module_src_map.ansi_port_decl
    }
}
