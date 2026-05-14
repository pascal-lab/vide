use slang::{
    SyntaxToken,
    ast::{
        AstNode, BlockStatement, ConfigDeclaration, Declarator, FunctionDeclaration,
        HierarchicalInstance, IdentifierName, ModuleDeclaration, NonAnsiPort, ParamAssignment,
        PortConnection, PortReference, SpecparamDeclarator, Statement,
    },
};

pub trait HasName<'a>: AstNode<'a> {
    fn name(&self) -> Option<SyntaxToken<'a>>;
}

impl<'a> HasName<'a> for ModuleDeclaration<'a> {
    fn name(&self) -> Option<SyntaxToken<'a>> {
        self.header().name()
    }
}

impl<'a> HasName<'a> for ConfigDeclaration<'a> {
    fn name(&self) -> Option<SyntaxToken<'a>> {
        self.name()
    }
}

impl<'a> HasName<'a> for BlockStatement<'a> {
    fn name(&self) -> Option<SyntaxToken<'a>> {
        self.block_name()?.name()
    }
}

impl<'a> HasName<'a> for NonAnsiPort<'a> {
    fn name(&self) -> Option<SyntaxToken<'a>> {
        self.as_explicit_non_ansi_port()?.name()
    }
}

impl<'a> HasName<'a> for PortReference<'a> {
    fn name(&self) -> Option<SyntaxToken<'a>> {
        self.name()
    }
}

impl<'a> HasName<'a> for Declarator<'a> {
    fn name(&self) -> Option<SyntaxToken<'a>> {
        self.name()
    }
}

impl<'a> HasName<'a> for IdentifierName<'a> {
    fn name(&self) -> Option<SyntaxToken<'a>> {
        self.identifier()
    }
}

impl<'a> HasName<'a> for SpecparamDeclarator<'a> {
    fn name(&self) -> Option<SyntaxToken<'a>> {
        self.name()
    }
}

impl<'a> HasName<'a> for ParamAssignment<'a> {
    fn name(&self) -> Option<SyntaxToken<'a>> {
        match self {
            ParamAssignment::OrderedParamAssignment(_) => None,
            ParamAssignment::NamedParamAssignment(assign) => assign.name(),
        }
    }
}

impl<'a> HasName<'a> for PortConnection<'a> {
    fn name(&self) -> Option<SyntaxToken<'a>> {
        match self {
            PortConnection::NamedPortConnection(conn) => conn.name(),
            _ => None,
        }
    }
}
impl<'a> HasName<'a> for HierarchicalInstance<'a> {
    fn name(&self) -> Option<SyntaxToken<'a>> {
        self.decl()?.name()
    }
}

impl<'a> HasName<'a> for Statement<'a> {
    fn name(&self) -> Option<SyntaxToken<'a>> {
        self.label()?.name()
    }
}

impl<'a> HasName<'a> for FunctionDeclaration<'a> {
    fn name(&self) -> Option<SyntaxToken<'a>> {
        fn rightmost_name_token(name: slang::ast::Name<'_>) -> Option<SyntaxToken<'_>> {
            if let Some(name) = name.as_identifier_name() {
                return name.identifier();
            }
            if let Some(name) = name.as_identifier_select_name() {
                return name.identifier();
            }
            if let Some(name) = name.as_scoped_name() {
                return rightmost_name_token(name.right());
            }
            None
        }

        rightmost_name_token(self.prototype().name())
    }
}
