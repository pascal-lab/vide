include!(concat!(env!("OUT_DIR"), "/syntax_kind.rs"));

impl SyntaxKind {
    pub fn as_u16(self) -> u16 {
        self.0
    }
}

impl TokenKind {
    pub fn as_u16(self) -> u16 {
        self.0
    }
}

pub mod cursor;
pub mod iter;
