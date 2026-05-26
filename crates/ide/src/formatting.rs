use std::{
    iter,
    ops::{ControlFlow, Range},
    process::{Command, Stdio},
    str,
};

use anyhow::Context as _;
use base_db::source_db::SourceDb;
use dissimilar::Chunk;
use hir::semantics::Semantics;
use ide_db::root_db::RootDb;
use itertools::Itertools;
use span::FilePosition;
use syntax::{
    SyntaxCursor, SyntaxCursorExt, SyntaxKind, SyntaxTrivia, Trivia, has_text_range::HasTextRange,
    token::SyntaxTokenWithParentExt, trivia::TriviaKindExt,
};
use utils::{
    cancellation::CancellationToken,
    line_index::{TextRange, TextSize},
    lines::{LineEnding, LineInfo},
    paths::Utf8PathBuf,
    process::{configure_process_tree, wait_with_input_and_output_and_cancellation},
    text_edit::TextEdit,
};
use vfs::FileId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FormatterProvider {
    Verible,
}

impl FormatterProvider {
    fn supports_range_formatting(self) -> bool {
        matches!(self, Self::Verible)
    }
}

#[derive(Debug, Clone)]
pub struct FmtConfig {
    pub provider: FormatterProvider,
    pub executable: Option<Utf8PathBuf>,
    pub args: Vec<String>,
    pub indent_width: usize,
    pub on_enter: bool,
    pub in_comments: bool,
}

impl FmtConfig {
    pub fn apply_editor_options(&mut self, tab_size: u32, _insert_spaces: bool) {
        self.indent_width = tab_size as usize;
    }
}

pub(crate) fn format(
    db: &RootDb,
    file_id: FileId,
    line_range: Option<Range<usize>>,
    LineInfo { ending, .. }: &LineInfo,
    config: FmtConfig,
    cancellation: &CancellationToken,
) -> anyhow::Result<Option<TextEdit>> {
    let text = db.file_text(file_id);
    format_inner(text.as_ref(), line_range, ending, config, cancellation)
}

fn format_inner(
    text: &str,
    line_range: Option<Range<usize>>,
    ending: &LineEnding,
    config: FmtConfig,
    cancellation: &CancellationToken,
) -> Result<Option<TextEdit>, anyhow::Error> {
    cancellation.check()?;
    let new_text = match config.provider {
        FormatterProvider::Verible => format_verible(text, line_range, &config, cancellation)?,
    };
    cancellation.check()?;

    let (new_text, new_line_endings) = LineEnding::normalize(new_text);

    if *ending != new_line_endings {
        let range = TextRange::up_to(TextSize::of(text));
        Ok(Some(TextEdit::replace(range, new_text)))
    } else if *text == new_text {
        Ok(None)
    } else {
        Ok(Some(diff(text, &new_text)))
    }
}

fn format_verible(
    text: &str,
    line_range: Option<Range<usize>>,
    config: &FmtConfig,
    cancellation: &CancellationToken,
) -> anyhow::Result<String> {
    let verible_fmt_path = config
        .executable
        .clone()
        .map_or_else(|| which::which("verible-verilog-format"), |p| Ok(p.into()))?;

    let mut cmd = Command::new(verible_fmt_path);

    cmd.args(&config.args);
    cmd.arg(format!("--indentation_spaces={}", config.indent_width));
    if let Some(lines) = line_range {
        cmd.arg("--lines").arg(format!("{}-{}", lines.start + 1, lines.end));
    }
    configure_process_tree(&mut cmd);

    cancellation.check()?;
    let fmt =
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).arg("-").spawn()?;

    let output =
        wait_with_input_and_output_and_cancellation(fmt, text.as_bytes().to_vec(), cancellation)?;

    if !output.status.success() {
        let stderr = String::from_utf8(output.stderr)?;
        return Err(anyhow::format_err!("verible-verilog-format failed: {}", stderr));
    }

    String::from_utf8(output.stdout).context("verible-verilog-format produced invalid UTF-8")
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
    cancellation: &CancellationToken,
) -> anyhow::Result<Option<TextEdit>> {
    cancellation.check()?;
    if ch.as_str() != "\n" {
        return Ok(None);
    }

    let sema = Semantics::new(db);
    let parsed_file = sema.parse_file(file_id);
    let Some(root) = parsed_file.root() else {
        return Ok(None);
    };

    let mut cursor = root.walk();

    cursor.goto_first_tok_after_or_last(offset);
    let Some(right) = cursor.to_tok_with_parent() else {
        return Ok(None);
    };
    let trivias = right.trivias_with_range().collect_vec();
    let idx = trivias.iter().position(|(range, _)| range.contains(offset));

    // region: inside comments
    if config.in_comments {
        if let Some(idx) = idx
            && let Some((range, trivia)) = trivias.get(idx)
            && trivia.kind().is_bc()
        {
            return format_in_bc(*trivia, range.start(), offset);
        }

        if let Some(edits) = format_in_lc(&trivias, idx.unwrap_or(trivias.len()), offset) {
            return Ok(Some(edits));
        }
    }
    // endregion

    let mut res = TextEdit::default();

    // region: formatting
    if config.on_enter
        && config.provider.supports_range_formatting()
        && let Some(trivias) = trivias.get(..idx.unwrap_or(trivias.len()))
        && let Some(edits) =
            format_previous(db, file_id, trivias, &mut cursor, line_info, config, cancellation)
    {
        res.union(edits)
            .map_err(|_| anyhow::format_err!("on-type formatting produced overlapping edits"))?;
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
        .filter(|(_, t)| t.kind().is_whitespace())
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
    let text = str::from_utf8(comment.get_raw_text().as_bytes())?;

    let Some((prev, line_start)) = text
        .lines()
        .try_fold((block_start, None), |(line_start, prev), line| {
            let end = line_start + TextSize::from(line.len() as u32) + TextSize::from(1);
            if offset < end {
                ControlFlow::Break(prev.map(|prev| (prev, line_start)))
            } else {
                ControlFlow::Continue((end, Some(line)))
            }
        })
        .break_value()
        .flatten()
    else {
        return Ok(None);
    };

    if prev.trim().starts_with("/*") {
        return Ok(None);
    }

    let indent = prev.chars().take_while(|&c| c == ' ').count();
    let mut res = String::with_capacity(prev.len() + 2);
    res.extend(iter::repeat_n(' ', indent));
    if prev[indent..].strip_prefix('*').is_some() {
        res.push_str("* ");
    }

    Ok(Some(TextEdit::replace(TextRange::new(line_start, offset), res)))
}

const PLACEHOLDER: &str = "/**/"; // used for separating ranges of edits

fn format_previous<'a>(
    db: &RootDb,
    file_id: FileId,
    trivias: &[(TextRange, SyntaxTrivia<'a>)],
    cursor: &mut SyntaxCursor<'a>,
    LineInfo { ending, index, .. }: &LineInfo,
    config: FmtConfig,
    cancellation: &CancellationToken,
) -> Option<TextEdit> {
    check!(trivias.iter().filter(|(_, t)| t.kind().is_eol()).count() == 1);

    let offset = trivias.last()?.0.end();
    cursor.reset_to_root();
    check!(cursor.goto_last_tok_before(offset));

    let list_range = loop {
        check!(cursor.goto_parent());

        let node = cursor.to_node()?;
        if matches!(node.kind(), SyntaxKind::SYNTAX_LIST | SyntaxKind::SEPARATED_LIST) {
            check!(cursor.goto_last_child_before_pos(offset.into()));

            if let Some(last_child) = cursor.to_node()
                && last_child.text_range().is_some_and(|range| range.contains(offset))
            {
                // Inside the element
                return None;
            }

            break node.text_range()?;
        }
    };

    let line_range = index.line_ranges(list_range);

    let mut text = db.file_text(file_id).to_string();
    text.insert_str(offset.into(), PLACEHOLDER);

    // WORKAROUND: if there is a redundant token at the end of the list, remove it.
    // Otherwise, the formatter will not work correctly
    if let Some(token) = cursor.to_tok_with_parent()
        && let Some(token_range) = token.text_range()
        && let Some(idx) = cursor.idx()
    {
        cursor.goto_parent();
        if cursor.to_node().is_some_and(|node| node.child_count() == idx + 1) {
            text.replace_range(Range::<usize>::from(token_range), "");
        }
    }

    let Ok(Some(edits)) = format_inner(&text, line_range, ending, config, cancellation) else {
        return None;
    };

    let edits = edits.into_iter().filter(|edit| edit.del.end() <= offset).collect();

    Some(edits)
}

#[cfg(test)]
mod tests {
    use base_db::{change::Change, source_root::SourceRoot};
    use ide_db::{line_index_db::LineIndexDb, root_db::RootDb};
    use span::FilePosition;
    use triomphe::Arc;
    use utils::{
        cancellation::CancellationToken,
        lines::{LineEnding, LineInfo, PositionEncoding},
        text_edit::TextSize,
    };
    use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

    use super::{FmtConfig, FormatterProvider, format_on_type};

    fn db_with_file(text: &str) -> (RootDb, FileId) {
        let file_id = FileId(0);
        let path = VfsPath::new_virtual_path("/test.sv".to_owned());

        let mut file_set = FileSet::default();
        file_set.insert(file_id, path);
        let root = SourceRoot::new_local(file_set);

        let mut change = Change::new();
        change.set_roots(vec![root]);
        change.add_changed_file(ChangedFile {
            file_id,
            change_kind: ChangeKind::Create(Arc::from(text), LineEnding::Unix),
        });

        let mut db = RootDb::new(None);
        change.apply(&mut db);
        (db, file_id)
    }

    fn line_info(db: &RootDb, file_id: FileId) -> LineInfo {
        LineInfo {
            index: db.line_index(file_id),
            ending: LineEnding::Unix,
            encoding: PositionEncoding::Utf8,
        }
    }

    fn config() -> FmtConfig {
        FmtConfig {
            provider: FormatterProvider::Verible,
            executable: None,
            args: Vec::new(),
            indent_width: 4,
            on_enter: false,
            in_comments: true,
        }
    }

    #[test]
    fn unsupported_on_type_trigger_is_no_edit() {
        let (db, file_id) = db_with_file("module A;\nendmodule");
        let edit = format_on_type(
            &db,
            FilePosition { file_id, offset: TextSize::from(0) },
            ".".to_owned(),
            &line_info(&db, file_id),
            config(),
            &CancellationToken::new(),
        )
        .unwrap();

        assert!(edit.is_none());
    }

    #[test]
    fn first_line_inside_block_comment_is_no_edit() {
        let text = "/*\n*/";
        let (db, file_id) = db_with_file(text);
        let edit = format_on_type(
            &db,
            FilePosition { file_id, offset: TextSize::from(3) },
            "\n".to_owned(),
            &line_info(&db, file_id),
            config(),
            &CancellationToken::new(),
        )
        .unwrap();

        assert!(edit.is_none());
    }
}
