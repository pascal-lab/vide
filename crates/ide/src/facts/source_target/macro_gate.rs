use utils::line_index::TextSize;

pub(super) fn source_macro_invocation_may_cover_offset(text: &str, offset: TextSize) -> bool {
    let offset = usize::from(offset);
    if offset > text.len() || !text.is_char_boundary(offset) {
        return false;
    }

    let search_end = text[offset..].chars().next().map_or(offset, |ch| offset + ch.len_utf8());
    let prefix = &text[..search_end];
    for (tick, _) in prefix.match_indices('`').rev() {
        match macro_invocation_candidate_end(text, tick) {
            MacroInvocationCandidate::RangeEnd(end) if offset <= end => return true,
            MacroInvocationCandidate::RangeEnd(_) => {}
            MacroInvocationCandidate::Malformed => return true,
            MacroInvocationCandidate::NotMacro => {}
        }
    }
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MacroInvocationCandidate {
    RangeEnd(usize),
    Malformed,
    NotMacro,
}

fn macro_invocation_candidate_end(text: &str, tick: usize) -> MacroInvocationCandidate {
    let Some(after_tick) = text.get(tick + 1..) else {
        return MacroInvocationCandidate::Malformed;
    };
    let Some((name_start_offset, first)) = after_tick.char_indices().next() else {
        return MacroInvocationCandidate::Malformed;
    };
    let name_start = tick + 1 + name_start_offset;
    let name_end = if first == '\\' {
        let Some((end, _)) = text[name_start..].char_indices().find(|(_, ch)| ch.is_whitespace())
        else {
            return MacroInvocationCandidate::Malformed;
        };
        name_start + end
    } else if is_macro_ident_start(first) {
        text[name_start..]
            .char_indices()
            .find_map(|(index, ch)| (!is_macro_ident_continue(ch)).then_some(name_start + index))
            .unwrap_or(text.len())
    } else {
        return MacroInvocationCandidate::NotMacro;
    };

    let after_name = &text[name_end..];
    let Some((next_offset, next)) = after_name.char_indices().find(|(_, ch)| !ch.is_whitespace())
    else {
        return MacroInvocationCandidate::RangeEnd(name_end);
    };
    if next != '(' {
        return MacroInvocationCandidate::RangeEnd(name_end);
    }
    let open = name_end + next_offset;
    match balanced_paren_end(text, open) {
        Some(end) => MacroInvocationCandidate::RangeEnd(end),
        None => MacroInvocationCandidate::Malformed,
    }
}

fn balanced_paren_end(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut chars = text[open..].char_indices();
    while let Some((relative, ch)) = chars.next() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + relative + ch.len_utf8());
                }
            }
            '"' => {
                while let Some((_, string_ch)) = chars.next() {
                    if string_ch == '\\' {
                        let _ = chars.next();
                    } else if string_ch == '"' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn is_macro_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_macro_ident_continue(ch: char) -> bool {
    is_macro_ident_start(ch) || ch.is_ascii_digit() || ch == '$'
}
