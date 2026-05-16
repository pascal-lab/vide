use utils::line_index::{TextRange, TextSize};

use crate::completion::{context::ExpectedSyntax, syntax_keywords};

const SOURCE_PREDICTED_ITEM_CONTEXTS: [ExpectedSyntax; 4] = [
    ExpectedSyntax::ConfigItem { rules_allowed: false },
    ExpectedSyntax::ConfigItem { rules_allowed: true },
    ExpectedSyntax::SpecifyItem,
    ExpectedSyntax::GenerateItem,
];

pub(crate) fn expected_item_syntax_from_source(
    source_text: &str,
    replacement: TextRange,
    offset: TextSize,
) -> Option<ExpectedSyntax> {
    let prefix = prefix_between(source_text, replacement.start(), offset)?;
    SOURCE_PREDICTED_ITEM_CONTEXTS.into_iter().find(|expected| {
        syntax_keywords::predicts_source_expected_keyword(
            *expected,
            source_text,
            replacement,
            prefix,
        )
    })
}

fn prefix_between(source_text: &str, start: TextSize, end: TextSize) -> Option<&str> {
    let start = usize::from(start);
    let end = usize::from(end);
    if start > end
        || end > source_text.len()
        || !source_text.is_char_boundary(start)
        || !source_text.is_char_boundary(end)
    {
        return None;
    }

    Some(&source_text[start..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predicts_recoverable_item_contexts_from_source() {
        let cases = [
            (
                ExpectedSyntax::SpecifyItem,
                "module m; specify\n  sp/*caret*/\nendspecify endmodule\n",
            ),
            (
                ExpectedSyntax::GenerateItem,
                "module m; generate\n  wi/*caret*/\nendgenerate endmodule\n",
            ),
            (
                ExpectedSyntax::ConfigItem { rules_allowed: false },
                "config cfg;\n  de/*caret*/\n  design work.top;\nendconfig\n",
            ),
            (
                ExpectedSyntax::ConfigItem { rules_allowed: true },
                "config cfg;\n  design work.top;\n  de/*caret*/\nendconfig\n",
            ),
        ];

        for (expected, text) in cases {
            let (source, offset, replacement) = source_with_caret(text);
            assert_eq!(
                expected_item_syntax_from_source(&source, replacement, offset),
                Some(expected)
            );
        }
    }

    fn source_with_caret(text: &str) -> (String, TextSize, TextRange) {
        let marker = "/*caret*/";
        let offset = text.find(marker).expect("missing caret");
        let source = text.replace(marker, "");
        let start = source[..offset]
            .rfind(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '$')
            .map_or(0, |idx| idx + 1);
        let replacement =
            TextRange::new(TextSize::from(start as u32), TextSize::from(offset as u32));
        (source, TextSize::from(offset as u32), replacement)
    }
}
