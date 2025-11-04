use slang::{
    SyntaxKind, SyntaxNode, SyntaxToken,
    ast::{self, AstNode, ModuleDeclaration, ModuleHeader, PackageImportDeclaration, SyntaxList},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PackageDeclaration<'a> {
    inner: ast::ModuleDeclaration<'a>,
}

impl<'a> PackageDeclaration<'a> {
    #[inline]
    pub fn from_module(decl: ast::ModuleDeclaration<'a>) -> Option<Self> {
        matches!(decl, ModuleDeclaration::PackageDeclaration(_)).then_some(Self { inner: decl })
    }

    #[inline]
    pub fn header(&self) -> PackageHeader<'a> {
        PackageHeader::new(self.inner.header()).expect("package header")
    }

    #[inline]
    pub fn members(&self) -> SyntaxList<'a, ast::Member<'a>> {
        self.inner.members()
    }

    #[inline]
    pub fn imports(&self) -> SyntaxList<'a, PackageImportDeclaration<'a>> {
        self.inner.header().imports()
    }

    #[inline]
    pub fn endpackage(&self) -> Option<SyntaxToken<'a>> {
        self.inner.endmodule()
    }

    #[inline]
    pub fn block_name(&self) -> Option<ast::NamedBlockClause<'a>> {
        self.inner.block_name()
    }

    #[inline]
    pub fn into_module(self) -> ast::ModuleDeclaration<'a> {
        self.inner
    }
}

impl<'a> AstNode<'a> for PackageDeclaration<'a> {
    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PACKAGE_DECLARATION
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        let decl = ModuleDeclaration::cast(syntax)?;
        Self::from_module(decl)
    }

    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.inner.syntax()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PackageHeader<'a> {
    inner: ast::ModuleHeader<'a>,
}

impl<'a> PackageHeader<'a> {
    #[inline]
    pub fn name(&self) -> Option<SyntaxToken<'a>> {
        self.inner.name()
    }

    #[inline]
    pub fn lifetime(&self) -> Option<SyntaxToken<'a>> {
        self.inner.lifetime()
    }

    #[inline]
    pub fn imports(&self) -> SyntaxList<'a, PackageImportDeclaration<'a>> {
        self.inner.imports()
    }

    #[inline]
    pub fn semi(&self) -> Option<SyntaxToken<'a>> {
        self.inner.semi()
    }

    #[inline]
    pub fn into_header(self) -> ast::ModuleHeader<'a> {
        self.inner
    }
}

impl<'a> PackageHeader<'a> {
    #[inline]
    pub fn new(inner: ast::ModuleHeader<'a>) -> Option<Self> {
        matches!(inner, ModuleHeader::PackageHeader(_)).then_some(Self { inner })
    }
}

impl<'a> AstNode<'a> for PackageHeader<'a> {
    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::PACKAGE_HEADER
    }

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        let header = ModuleHeader::cast(syntax)?;
        Self::new(header)
    }

    #[inline]
    fn syntax(&self) -> SyntaxNode<'a> {
        self.inner.syntax()
    }
}
