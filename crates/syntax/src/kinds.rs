include!("generated/kinds.rs");

impl SyntaxKind {
    pub const fn as_u16(self) -> u16 {
        self.0
    }
}

impl TokenKind {
    pub const fn as_u16(self) -> u16 {
        self.0
    }
}

impl TriviaKind {
    pub const fn as_u8(self) -> u8 {
        self.0
    }
}
