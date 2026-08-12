//! CAPI2 conditional expression parser and evaluator.
//!
//! FuseSoC core files allow string values to contain conditional expressions
//! using the syntax:
//!
//! ```text
//! exprs       ::= expr+
//! expr        ::= word | conditional
//! conditional ::= ["!"] word "?" "(" exprs ")"
//! word        ::= [a-zA-Z0-9:<>.\[\]_-,=~/^+"$]+
//! ```
//!
//! A conditional `foo ? (bar)` evaluates to `bar` when flag `foo` is set.
//! `!foo ? (bar)` evaluates to `bar` when `foo` is NOT set.  Bare words are
//! always included.
//!
//! The expanded result is a space-joined string (or list of words).

use std::fmt;

/// A parsed expression — a sequence of parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprPart {
    /// A literal word.
    Word(String),
    /// A conditional: `flag ? (body)` or `!flag ? (body)`.
    Conditional {
        negated: bool,
        flag: String,
        body: Vec<ExprPart>,
    },
}

/// A set of active flags (from target flags, tool selection, etc.).
pub type FlagDefs = std::collections::HashSet<String>;

/// Parse a CAPI2 expression string into a list of [`ExprPart`]s.
///
/// Returns an error if the syntax is invalid.
pub fn parse(input: &str) -> Result<Vec<ExprPart>, ExprParseError> {
    let mut parser = ExprParser::new(input);
    let parts = parser.parse_exprs()?;
    if !parser.at_end() {
        return Err(parser.error("unexpected trailing characters"));
    }
    Ok(parts)
}

/// Expand a parsed expression with the given flag definitions.
///
/// Returns the expanded words in order.
pub fn expand(parts: &[ExprPart], flags: &FlagDefs) -> Vec<String> {
    let mut out = Vec::new();
    for part in parts {
        match part {
            ExprPart::Word(w) => out.push(w.clone()),
            ExprPart::Conditional { negated, flag, body } => {
                let active = flags.contains(flag);
                if active != *negated {
                    // Condition is true — expand the body.
                    out.extend(expand(body, flags));
                }
                // Condition is false — skip.
            }
        }
    }
    out
}

/// Parse and expand in one step.
pub fn parse_and_expand(input: &str, flags: &FlagDefs) -> Result<Vec<String>, ExprParseError> {
    let parts = parse(input)?;
    Ok(expand(&parts, flags))
}

/// Expand a single string, joining words with spaces.  If the string contains
/// no conditionals, returns it as-is.
pub fn expand_string(input: &str, flags: &FlagDefs) -> Result<String, ExprParseError> {
    let words = parse_and_expand(input, flags)?;
    Ok(words.join(" "))
}

/// Check if a string contains any conditional expressions.
pub fn has_conditionals(input: &str) -> bool {
    input.contains('?')
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Character classes allowed in a "word".
const WORD_CHARS: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789:`<>[].[]_-,=~/^+\"$";

#[derive(Debug)]
pub struct ExprParseError {
    pub message: String,
    pub position: usize,
}

impl fmt::Display for ExprParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "expression parse error at position {}: {}", self.position, self.message)
    }
}

impl std::error::Error for ExprParseError {}

struct ExprParser<'a> {
    chars: Vec<char>,
    pos: usize,
    _input: &'a str,
}

impl<'a> ExprParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
            _input: input,
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek();
        self.pos += 1;
        c
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn error(&self, msg: impl Into<String>) -> ExprParseError {
        ExprParseError {
            message: msg.into(),
            position: self.pos,
        }
    }

    fn parse_exprs(&mut self) -> Result<Vec<ExprPart>, ExprParseError> {
        let mut parts = Vec::new();
        loop {
            self.skip_ws();
            if self.at_end() {
                break;
            }
            // Stop at ')' — we're inside a conditional and ')' closes it.
            if self.peek() == Some(')') {
                break;
            }
            // Check for conditional: ["!"] word "?(" exprs ")"
            let start = self.pos;
            let part = self.parse_expr()?;
            parts.push(part);
            // Avoid infinite loop on empty match.
            if self.pos == start {
                break;
            }
        }
        Ok(parts)
    }

    fn parse_expr(&mut self) -> Result<ExprPart, ExprParseError> {
        self.skip_ws();
        // Try conditional: ["!"] word "?(" exprs ")"
        let save = self.pos;

        let negated = if self.peek() == Some('!') {
            self.advance();
            self.skip_ws();
            true
        } else {
            false
        };

        // Read the flag word (stopping at whitespace or '?').
        let flag = self.read_word_until_cond_or_ws();

        if flag.is_empty() {
            // Not a conditional — restore and read as word.
            self.pos = save;
            let w = self.read_word_general();
            if w.is_empty() {
                return Err(self.error("expected a word or conditional"));
            }
            return Ok(ExprPart::Word(w));
        }

        self.skip_ws();

        // Check for "?" followed by "(".
        if self.peek() == Some('?') {
            self.advance();
            self.skip_ws();
            if self.peek() == Some('(') {
                self.advance();
                let body = self.parse_exprs()?;
                self.skip_ws();
                if self.peek() != Some(')') {
                    return Err(self.error("expected ')' to close conditional"));
                }
                self.advance();
                return Ok(ExprPart::Conditional { negated, flag, body });
            }
            // "?" without "(" — backtrack and parse as plain word.
        }

        // Not a conditional. Restore position and parse as word.
        self.pos = save;
        let w = self.read_word_general();
        if w.is_empty() {
            return Err(self.error("expected a word"));
        }
        Ok(ExprPart::Word(w))
    }

    /// Read a word, stopping at whitespace.  '?' is included in the word
    /// unless it is immediately followed by '(' (conditional marker).
    fn read_word_general(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                break;
            }
            // Stop at '?' if it's followed by '(' — that's a conditional.
            if c == '?' && self.chars.get(self.pos + 1).copied() == Some('(') {
                break;
            }
            if WORD_CHARS.contains(c) || c == '?' {
                s.push(c);
                self.pos += 1;
            } else {
                break;
            }
        }
        s
    }

    /// Read a word stopping at whitespace or '?' (for conditional flag reading).
    fn read_word_until_cond_or_ws(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_whitespace() || c == '?' {
                break;
            }
            if WORD_CHARS.contains(c) {
                s.push(c);
                self.pos += 1;
            } else {
                break;
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flags(items: &[&str]) -> FlagDefs {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn plain_word() {
        let parts = parse("rtl").unwrap();
        assert_eq!(parts, vec![ExprPart::Word("rtl".into())]);
        assert_eq!(expand(&parts, &flags(&[])), vec!["rtl"]);
    }

    #[test]
    fn multiple_words() {
        let parts = parse("rtl tb").unwrap();
        assert_eq!(parts, vec![ExprPart::Word("rtl".into()), ExprPart::Word("tb".into())]);
        assert_eq!(expand(&parts, &flags(&[])), vec!["rtl", "tb"]);
    }

    #[test]
    fn conditional_true() {
        let parts = parse("tool_icarus ? (rtl)").unwrap();
        assert_eq!(
            parts,
            vec![ExprPart::Conditional {
                negated: false,
                flag: "tool_icarus".into(),
                body: vec![ExprPart::Word("rtl".into())],
            }]
        );
        assert_eq!(expand(&parts, &flags(&["tool_icarus"])), vec!["rtl"]);
    }

    #[test]
    fn conditional_false() {
        let parts = parse("tool_icarus ? (rtl)").unwrap();
        assert!(expand(&parts, &flags(&[])).is_empty());
    }

    #[test]
    fn negated_conditional() {
        let parts = parse("!synthesis ? (sim_only)").unwrap();
        assert_eq!(expand(&parts, &flags(&[])), vec!["sim_only"]);
        assert!(expand(&parts, &flags(&["synthesis"])).is_empty());
    }

    #[test]
    fn mixed_words_and_conditionals() {
        let parts = parse("common tool_verilator ? (rtl_verilator)").unwrap();
        assert_eq!(expand(&parts, &flags(&["tool_verilator"])), vec!["common", "rtl_verilator"]);
        assert_eq!(expand(&parts, &flags(&[])), vec!["common"]);
    }

    #[test]
    fn nested_conditional() {
        let parts = parse("a ? (b ? (c))").unwrap();
        assert_eq!(expand(&parts, &flags(&["a", "b"])), vec!["c"]);
        assert!(expand(&parts, &flags(&["a"])).is_empty());
        assert!(expand(&parts, &flags(&[])).is_empty());
    }

    #[test]
    fn expand_string_joins_with_space() {
        assert_eq!(expand_string("rtl tb", &flags(&[])).unwrap(), "rtl tb");
        assert_eq!(
            expand_string("tool_v ? (rtl_v) common", &flags(&["tool_v"])).unwrap(),
            "rtl_v common"
        );
    }
}