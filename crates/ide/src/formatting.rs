use std::{
    io::Write,
    iter,
    ops::ControlFlow,
    process::{Command, Stdio},
};

use base_db::source_db::SourceDb;
use dissimilar::Chunk;
use hir::semantics::Semantics;
use ide_db::root_db::RootDb;
use itertools::Itertools;
use line_index::{TextRange, TextSize};
use span::FilePosition;
use syntax::{
    SyntaxCursor, SyntaxCursorExt, SyntaxKind, SyntaxTrivia, Trivia,
    ast::AstNode,
    has_text_range::{HasTextRange, SourceRangeExt},
    token::SyntaxTokenExt,
    trivia::TriviaKindExt,
};
use utils::{
    lines::{LineEnding, LineInfo},
    paths::Utf8PathBuf,
    text_edit::TextEdit,
};
use vfs::FileId;

#[derive(Debug)]
pub struct FmtConfig {
    pub executable: Option<Utf8PathBuf>,
    pub args: Vec<String>,
    pub on_enter: bool,
    pub in_comments: bool,
}

pub(crate) fn format(
    db: &RootDb,
    file_id: FileId,
    line_range: Option<(usize, usize)>,
    LineInfo { ending, .. }: &LineInfo,
    config: FmtConfig,
) -> anyhow::Result<Option<TextEdit>> {
    let text = db.file_text(file_id);
    format_inner(text.as_ref(), line_range, ending, config)
}

fn format_inner(
    text: &str,
    line_range: Option<(usize, usize)>,
    ending: &LineEnding,
    config: FmtConfig,
) -> Result<Option<TextEdit>, anyhow::Error> {
    let verible_fmt_path = config
        .executable
        .map_or_else(|| which::which("verible-verilog-format"), |p| Ok(p.into()))?;

    let mut cmd = Command::new(verible_fmt_path);

    cmd.args(&config.args);
    if let Some((start_line, end_line)) = line_range {
        cmd.arg("--lines").arg(format!("{}-{}", start_line + 1, end_line + 1));
    }

    let mut fmt =
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).arg("-").spawn()?;

    fmt.stdin
        .as_mut()
        .ok_or(anyhow::format_err!("verible-verilog-format: could not open stdin"))?
        .write_all(text.as_bytes())?;

    let output = fmt.wait_with_output()?;

    if !output.status.success() {
        let stderr = String::from_utf8(output.stderr)?;
        return Err(anyhow::format_err!("verible-verilog-format failed: {}", stderr));
    }

    let (new_text, new_line_endings) = LineEnding::normalize(String::from_utf8(output.stdout)?);

    if *ending != new_line_endings {
        let range = TextRange::up_to(TextSize::of(text));
        Ok(Some(TextEdit::replace(range, new_text)))
    } else if *text == new_text {
        Ok(None)
    } else {
        Ok(Some(diff(text, &new_text)))
    }
}

fn diff(old: &str, new: &str) -> TextEdit {
    let mut builder = TextEdit::builder();
    let mut pos = TextSize::default();
    let mut chunks = dissimilar::diff(old, new).into_iter().peekable();

    while let Some(chunk) = chunks.next() {
        match chunk {
            Chunk::Equal(text) => pos += TextSize::of(text),
            Chunk::Delete(deleted) => {
                let deleted = TextSize::of(deleted);
                if let Some(&Chunk::Insert(inserted)) = chunks.peek() {
                    chunks.next();
                    builder.replace(TextRange::at(pos, deleted), inserted.into());
                } else {
                    builder.delete(TextRange::at(pos, deleted));
                }
                pos += deleted;
            }
            Chunk::Insert(inserted) => builder.insert(pos, inserted.into()),
        }
    }

    builder.finish()
}

macro_rules! check {
    ($trivia:expr, $kind:expr) => {
        if $trivia?.1.kind() != $kind {
            return None;
        }
    };
    ($b:expr) => {
        if !$b {
            return None;
        }
    };
}

pub fn format_on_type(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
    ch: String,
    line_info: &LineInfo,
    config: FmtConfig,
) -> anyhow::Result<Option<TextEdit>> {
    let sema = Semantics::new(db);
    let root = sema.parse(file_id).syntax();

    if ch.as_str() != "\n" {
        panic!("format_on_type: invalid character: {}", ch);
    }

    let mut cursor = root.walk();

    cursor.goto_first_tok_after_or_last(offset);
    let right = cursor.to_token().unwrap();
    let trivias = right.trivias_with_range().unwrap().collect_vec();
    let idx = trivias.iter().position(|(range, _)| range.contains(offset));

    // region: inside comments
    if config.in_comments {
        if let Some(idx) = idx
            && let (_, trivia) = trivias[idx]
            && trivia.kind() == Trivia![bc]
        {
            return format_in_bc(trivia, trivias[idx].0.start(), offset);
        }

        if let Some(edits) = format_in_lc(&trivias, idx.unwrap_or(trivias.len()), offset) {
            return Ok(Some(edits));
        }
    }
    // endregion

    let mut res = TextEdit::default();

    // region: formatting
    if config.on_enter
        && let trivias = &trivias[..idx.unwrap_or(trivias.len())]
        && let Some(edits) = format_previous(db, file_id, trivias, &mut cursor, line_info, config)
    {
        res.union(edits).unwrap();
    }
    // endregion

    Ok(if res.is_empty() { None } else { Some(res) })
}

fn format_in_lc(
    trivias: &[(TextRange, SyntaxTrivia<'_>)],
    idx: usize,
    offset: TextSize,
) -> Option<TextEdit> {
    /*          // xxx
     * xxx|  => |
     * yyy      // yyy
     */
    let mut prev_eol = idx.checked_sub(1)?;
    let line_start = if let Some((range, t)) = trivias.get(prev_eol)
        && t.kind().is_whitespace()
    {
        prev_eol = prev_eol.checked_sub(1)?;
        range.start()
    } else {
        offset
    };

    check!(trivias.get(prev_eol), Trivia![eol]);
    check!(trivias.get(prev_eol.checked_sub(1)?), Trivia![lc]);

    if idx < trivias.len() {
        let mut next_lc = idx + 1;
        if trivias.get(next_lc).is_some_and(|(_, t)| t.kind().is_whitespace()) {
            next_lc += 1;
        }
        check!(trivias.get(next_lc), Trivia![lc]);
    }

    let indent = prev_eol
        .checked_sub(2)
        .and_then(|idx| trivias.get(idx))
        .filter(|(_, t)| t.kind() == Trivia![ws])
        .map_or(0, |(range, _)| range.len().into());

    if let Some(indent_exists) = offset.checked_sub(line_start)
        && let Some(indent) = TextSize::from(indent as u32).checked_sub(indent_exists)
    {
        // It is better to insert text only, so that some editors like VS Code
        // will not blink
        let res = format!("{}// ", " ".repeat(indent.into()));
        Some(TextEdit::insert(offset, res))
    } else {
        let res = format!("{}// ", " ".repeat(indent));
        Some(TextEdit::replace(TextRange::new(line_start, offset), res))
    }
}

fn format_in_bc(
    comment: SyntaxTrivia<'_>,
    block_start: TextSize,
    offset: TextSize,
) -> anyhow::Result<Option<TextEdit>> {
    let text = comment.get_raw_text().to_string();

    let (prev, line_start) = text
        .lines()
        .try_fold((block_start, None), |(line_start, prev), line| {
            let end = line_start + TextSize::from(line.len() as u32) + TextSize::from(1);
            if offset < end {
                ControlFlow::Break((prev.unwrap(), line_start))
            } else {
                ControlFlow::Continue((end, Some(line)))
            }
        })
        .break_value()
        .unwrap();

    if prev.trim().starts_with("/*") {
        return Ok(None);
    }

    let mut res = String::with_capacity(prev.len());
    let indent = prev.chars().take_while(|&c| c == ' ').count();
    res.extend(iter::repeat(' ').take(indent));
    if prev[indent..].strip_prefix('*').is_some() {
        res.push_str("* ");
    }

    Ok(Some(TextEdit::replace(TextRange::new(line_start, offset), res)))
}

fn format_previous<'a>(
    db: &RootDb,
    file_id: FileId,
    trivias: &[(TextRange, SyntaxTrivia<'a>)],
    cursor: &mut SyntaxCursor<'a>,
    LineInfo { ending, index, .. }: &LineInfo,
    config: FmtConfig,
) -> Option<TextEdit> {
    check!(trivias.iter().filter(|(_, t)| t.kind().is_eol()).count() == 1);

    let offset = trivias.last().unwrap().0.end();
    cursor.reset_to_root();
    check!(cursor.goto_last_tok_before(offset));

    let list_range = loop {
        check!(cursor.goto_parent());
        let is_list = |kind| kind == SyntaxKind::SYNTAX_LIST || kind == SyntaxKind::SEPARATED_LIST;
        if is_list(cursor.to_node().unwrap().kind()) {
            let list = cursor.to_node().unwrap();

            check!(cursor.goto_last_child_before_pos(offset.into()));
            if let Some(node) = cursor.to_node()
                && node.text_range().unwrap().contains(offset)
            {
                // Inside the element
                return None;
            }

            break list.range().unwrap().to_text_range();
        }
    };

    let line_range = [list_range.start(), list_range.end()]
        .into_iter()
        .map(|pos| index.line_col(pos).line as usize)
        .collect_tuple();

    // TODO: add api for reparse the file?
    const PLACEHOLDER: &str = "/**/"; // used for separating ranges of edits
    let mut text = db.file_text(file_id).to_string();
    text.insert_str(offset.into(), PLACEHOLDER);
    dbg!(&list_range);
    let Ok(Some(edits)) = dbg!(format_inner(&text, line_range, ending, config)) else {
        return None;
    };

    let edits = edits.into_iter().filter(|edit| edit.del.end() <= offset).collect();

    Some(edits)
}
