use syntax::{SyntaxToken, TokenKind, ast};

use super::expr::data_ty::DataTy;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct NetType {
    pub kind: NetKind,
    pub ty: DataTy,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum NetKind {
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

pub(crate) fn lower_net_kind(tok: Option<SyntaxToken>) -> Option<NetKind> {
    let kind = match tok?.kind() {
        TokenKind::SUPPLY_0_KEYWORD => NetKind::Supply0,
        TokenKind::SUPPLY_1_KEYWORD => NetKind::Supply1,
        TokenKind::TRI_KEYWORD => NetKind::Tri,
        TokenKind::TRI_AND_KEYWORD => NetKind::Triand,
        TokenKind::TRI_OR_KEYWORD => NetKind::Trior,
        TokenKind::TRI_0_KEYWORD => NetKind::Tri0,
        TokenKind::TRI_1_KEYWORD => NetKind::Tri1,
        TokenKind::WIRE_KEYWORD => NetKind::Wire,
        TokenKind::W_AND_KEYWORD => NetKind::Wand,
        TokenKind::W_OR_KEYWORD => NetKind::Wor,
        TokenKind::U_WIRE_KEYWORD => NetKind::Uwire,
        _ => return None,
    };
    Some(kind)
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Strength {
    Supply,
    Strong,
    Pull,
    Weak,
    Highz,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct DriveStrength(pub Option<Strength>, pub Option<Strength>);

pub(crate) fn lower_strength(strength: SyntaxToken) -> Option<Strength> {
    let strength = match strength.kind() {
        TokenKind::SUPPLY_0_KEYWORD | TokenKind::SUPPLY_1_KEYWORD => Strength::Supply,
        TokenKind::STRONG_0_KEYWORD | TokenKind::STRONG_1_KEYWORD => Strength::Strong,
        TokenKind::PULL_0_KEYWORD | TokenKind::PULL_1_KEYWORD => Strength::Pull,
        TokenKind::WEAK_0_KEYWORD | TokenKind::WEAK_1_KEYWORD => Strength::Weak,
        TokenKind::HIGH_Z0_KEYWORD | TokenKind::HIGH_Z1_KEYWORD => Strength::Highz,
        _ => return None,
    };
    Some(strength)
}

pub(crate) fn lower_drive_strength(strength: ast::DriveStrength) -> DriveStrength {
    let strength0 = strength.strength_0().and_then(lower_strength);
    let strength1 = strength.strength_1().and_then(lower_strength);
    DriveStrength(strength0, strength1)
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum ChargeStrength {
    Small,
    Medium,
    Large,
}
