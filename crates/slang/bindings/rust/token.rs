include!(concat!(env!("OUT_DIR"), "/token.rs"));

#[macro_export]
macro_rules! Trivia {
    [ws] => { $crate::TriviaKind::WHITESPACE };
    [eol] => { $crate::TriviaKind::END_OF_LINE };
    [lc] => { $crate::TriviaKind::LINE_COMMENT };
    [bc] => { $crate::TriviaKind::BLOCK_COMMENT };
    ["`"] => { $crate::TriviaKind::DIRECTIVE };
}

#[cfg(test)]
mod tests {
    use crate::TokenKind;

    #[test]
    fn test_punctuation_tokens() {
        assert_eq!(Token!["'"], TokenKind::APOSTROPHE);
        assert_eq!(Token![:=], TokenKind::COLON_EQUALS);
        assert_eq!(Token![**], TokenKind::DOUBLE_STAR);
        assert_eq!(Token![>=], TokenKind::GREATER_THAN_EQUALS);
    }

    #[test]
    fn test_operator_tokens() {
        assert_eq!(Token![+], TokenKind::PLUS);
        assert_eq!(Token![+=], TokenKind::PLUS_EQUAL);
        assert_eq!(Token![->], TokenKind::MINUS_ARROW);
        assert_eq!(Token![&&], TokenKind::DOUBLE_AND);
    }

    #[test]
    fn test_keyword_tokens() {
        assert_eq!(Token![module], TokenKind::MODULE_KEYWORD);
        assert_eq!(Token![function], TokenKind::FUNCTION_KEYWORD);
        assert_eq!(Token![if], TokenKind::IF_KEYWORD);
        assert_eq!(Token![endmodule], TokenKind::END_MODULE_KEYWORD);
    }

    #[test]
    fn test_system_names() {
        assert_eq!(Token!["$unit"], TokenKind::UNIT_SYSTEM_NAME);
        assert_eq!(Token!["$root"], TokenKind::ROOT_SYSTEM_NAME);
    }

    #[test]
    fn test_macro_tokens() {
        assert_eq!(Token!["`\""], TokenKind::MACRO_QUOTE);
        assert_eq!(Token!["``"], TokenKind::MACRO_PASTE);
    }
}
