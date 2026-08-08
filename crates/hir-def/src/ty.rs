use base_db::salsa;
use syntax::{SyntaxKind, SyntaxToken, TokenKind, ast};
use triomphe::Arc;
use utils::text_edit::TextSize;

use super::expr::data_ty::DataTy;
use crate::{ast_id_map::SyntaxFileId, db::HirDefDb};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
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

/// `` `default_nettype `` directives of one file, in source order. `None`
/// marks `` `default_nettype none ``: implicit nets are illegal after that
/// point. The directive is consumed by the preprocessor but survives as a
/// structured trivia node, so no text parsing is involved.
#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn default_nettype_directives(
    db: &dyn HirDefDb,
    file: SyntaxFileId,
) -> Arc<[(TextSize, Option<NetKind>)]> {
    let file_id = file.hir_file(db);
    let tree = db.parse(file_id);
    let mut directives = Vec::new();
    let Some(root) = tree.root() else {
        return Arc::from(Vec::new());
    };
    for token in root.tokens() {
        for trivia in token.tok.trivias() {
            let Some(directive) = trivia.syntax() else { continue };
            if directive.kind() != SyntaxKind::DEFAULT_NET_TYPE_DIRECTIVE {
                continue;
            }
            // The directive node is trivia and has no root-buffer range, but
            // its leading `` `default_nettype `` token does; that offset is
            // where the directive takes effect.
            let Some(offset) = directive
                .child_token(0)
                .and_then(|token| token.range())
                .map(|range| TextSize::from(u32::try_from(range.start()).unwrap_or_default()))
            else {
                continue;
            };
            // `none` lexes as an unknown token or an identifier; both map to
            // `None` through `lower_net_kind`, meaning implicit nets illegal.
            let kind = directive.child_token(1).and_then(|tok| lower_net_kind(Some(tok)));
            directives.push((offset, kind));
        }
    }
    directives.sort_by_key(|(offset, _)| *offset);
    directives.dedup_by_key(|(offset, _)| *offset);
    Arc::from(directives)
}

pub(crate) fn set_default_nettype_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    default_nettype_directives::set_lru_capacity(db, capacity);
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
