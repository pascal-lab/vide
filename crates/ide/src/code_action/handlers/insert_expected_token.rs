use hir::base_db::source_db::SourceDb;
use utils::text_edit::TextSize;

use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, RepairKind,
};

const ID: CodeActionId = CodeActionId {
    name: "insert_expected_token",
    kind: CodeActionKind::Generate,
    repair: Some(RepairKind::InsertExpectedToken),
};

pub(super) fn insert_expected_token(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    if !ctx.allows_repair(RepairKind::InsertExpectedToken) {
        return None;
    }

    let (token, range) = ctx.diagnostics().items.iter().find_map(|diag| {
        let token = diag.expected_token.as_deref()?;
        Some((token, diag.range.unwrap_or_else(|| ctx.range())))
    })?;
    let offset = range.start();
    let text = ctx.sema().db.file_text(ctx.file_id());
    let insertion = expected_token_insertion(&text, offset, token);
    let label = format!("Insert missing '{token}'");

    collector.add(ID, label, ctx.range(), |builder| {
        builder.insert(offset, insertion);
    });

    Some(())
}

fn expected_token_insertion(text: &str, offset: TextSize, token: &str) -> String {
    if !word_like(token) {
        return token.to_owned();
    }

    let offset = usize::from(offset).min(text.len());
    let before = text[..offset].chars().next_back();
    let after = text[offset..].chars().next();
    let prefix = before.filter(|ch| word_boundary_required(*ch)).map(|_| " ").unwrap_or("");
    let suffix = after.filter(|ch| word_boundary_required(*ch)).map(|_| " ").unwrap_or("");
    format!("{prefix}{token}{suffix}")
}

fn word_like(text: &str) -> bool {
    text.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$'))
}

fn word_boundary_required(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$')
}

#[cfg(test)]
mod tests {
    use utils::text_edit::TextSize;

    use super::expected_token_insertion;

    #[test]
    fn expected_token_insertion_separates_keywords_from_names() {
        let text = "moduleendmodule";

        assert_eq!(
            expected_token_insertion(text, TextSize::from("module".len() as u32), "end"),
            " end "
        );
    }

    #[test]
    fn expected_token_insertion_keeps_punctuation_tight() {
        assert_eq!(expected_token_insertion("logic a", TextSize::from(7), ";"), ";");
    }
}
