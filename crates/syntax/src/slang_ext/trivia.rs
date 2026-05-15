use core::str;

use slang::{SyntaxTrivia, Trivia, TriviaKind};
use smol_str::{SmolStr, ToSmolStr};

pub trait TriviaKindExt {
    fn is_whitespace(&self) -> bool;
    fn is_eol(&self) -> bool;
    fn is_comment(&self) -> bool;
    fn is_lc(&self) -> bool;
    fn is_bc(&self) -> bool;
}

impl TriviaKindExt for TriviaKind {
    fn is_whitespace(&self) -> bool {
        matches!(*self, Trivia![ws])
    }

    fn is_eol(&self) -> bool {
        matches!(*self, Trivia![eol])
    }

    fn is_comment(&self) -> bool {
        matches!(*self, Trivia![bc] | Trivia![lc])
    }

    fn is_lc(&self) -> bool {
        matches!(*self, Trivia![lc])
    }

    fn is_bc(&self) -> bool {
        matches!(*self, Trivia![bc])
    }
}

pub trait TriviaExt {
    fn is_region_begin(&self) -> Option<Option<SmolStr>>;
    fn is_region_end(&self) -> bool;
    fn as_comment(&self) -> Option<&str>;
}

const REGION_BEGIN: &str = "region";
const REGION_END: &str = "endregion";

impl TriviaExt for SyntaxTrivia<'_> {
    #[inline]
    fn is_region_begin(&self) -> Option<Option<SmolStr>> {
        if !matches!(self.kind(), Trivia![lc]) {
            return None;
        }

        let bytes = self.get_raw_text().as_bytes();
        debug_assert!(str::from_utf8(bytes).is_ok());

        let text = unsafe { str::from_utf8_unchecked(bytes) };
        let text = text.strip_prefix("//")?.trim().strip_prefix(REGION_BEGIN)?;
        if text.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
            return None;
        }
        let caption = text.strip_prefix(":").unwrap_or(text).trim();

        if caption.is_empty() {
            return Some(None);
        }

        Some(Some(caption.to_smolstr()))
    }

    #[inline]
    fn is_region_end(&self) -> bool {
        if !matches!(self.kind(), Trivia![lc]) {
            return false;
        }

        let bytes = self.get_raw_text().as_bytes();
        debug_assert!(str::from_utf8(bytes).is_ok());

        let text = unsafe { str::from_utf8_unchecked(bytes) };
        text.strip_prefix("//").is_some_and(|text| text.trim_start().starts_with(REGION_END))
    }

    #[inline]
    fn as_comment(&self) -> Option<&str> {
        let s = self.get_raw_text().as_bytes();
        debug_assert!(str::from_utf8(s).is_ok());

        let s = unsafe { str::from_utf8_unchecked(s) };

        match self.kind() {
            Trivia![lc] => Some(s.strip_prefix("//")?.trim()),
            Trivia![bc] => Some(s.strip_prefix("/*")?.strip_suffix("*/")?.trim()),
            _ => None,
        }
    }
}
