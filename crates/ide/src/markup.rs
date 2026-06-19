use std::fmt;

const HOVER_BLOCK_DIVIDER: &str = "---";

#[derive(Clone, Default, Debug, Hash, PartialEq, Eq)]
pub struct Markup {
    text: String,
}

impl From<Markup> for String {
    fn from(markup: Markup) -> Self {
        markup.text
    }
}

impl From<String> for Markup {
    fn from(text: String) -> Self {
        Markup { text }
    }
}

impl fmt::Display for Markup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.text, f)
    }
}

impl Markup {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn merge(&mut self, other: Markup) {
        self.text.push_str(&other.text);
    }

    pub fn print(&mut self, contents: &str) {
        self.text.push_str(contents);
    }

    pub fn print_with_strong(&mut self, contents: &str) {
        self.text.push_str("**");
        self.text.push_str(contents);
        self.text.push_str("**");
    }

    pub fn println(&mut self, contents: &str) {
        self.text.push_str(contents);
    }

    pub fn title(&mut self, contents: &str) {
        self.text.push_str(contents);
        self.newline();
    }

    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }

    pub fn newline(&mut self) {
        self.text.push_str("\n\n");
    }

    pub fn horizontal_line(&mut self) {
        self.text.push_str("\n\n");
        self.text.push_str(HOVER_BLOCK_DIVIDER);
        self.text.push_str("\n\n");
    }

    pub fn section(&mut self, title: &str) {
        self.horizontal_line();
        self.text.push_str(title);
        self.text.push('\n');
    }

    pub fn metadata_line(&mut self, contents: &str) {
        self.horizontal_line();
        self.text.push_str(contents);
    }

    pub fn new_section(&mut self, title: &str) {
        self.text.push_str("\n## ");
        self.text.push_str(title);
        self.text.push_str("\n\n");
    }

    pub fn new_subsection(&mut self, title: &str) {
        self.text.push_str("\n### ");
        self.text.push_str(title);
        self.text.push_str("\n\n");
    }

    pub fn push_with_plain_fence(&mut self, contents: &str) {
        self.text.push_str("```\n");
        self.text.push_str(contents);
        self.text.push_str("\n```\n");
    }

    pub fn push_with_code_fence(&mut self, contents: &str) {
        self.text.push_str("```systemverilog\n"); // hmmm, the highlighting for systemverilog is poor...
        self.text.push_str(contents);
        self.text.push_str("\n```\n");
    }

    pub fn push_with_backticks(&mut self, contents: &str) {
        self.text.push_str(&inline_code(contents));
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

pub(crate) fn inline_code(contents: &str) -> String {
    let delimiter_len = max_backtick_run(contents).saturating_add(1);
    let delimiter = "`".repeat(delimiter_len);
    let padding = if contents.contains('`') { " " } else { "" };
    format!("{delimiter}{padding}{contents}{padding}{delimiter}")
}

pub(crate) fn markdown_link(label: &str, destination: &str) -> String {
    format!("[{}](<{}>)", markdown_link_label(label), markdown_link_destination(destination))
}

pub(crate) fn display_hover_path(path: impl Into<String>) -> String {
    path.into().replace('\\', "/")
}

pub(crate) fn display_project_path(path: impl Into<String>) -> String {
    let mut path = display_hover_path(path);
    while path.starts_with('/') {
        path.remove(0);
    }
    path
}

pub(crate) fn file_link_target(path: &str) -> String {
    let path = display_hover_path(path.to_owned());
    if path.starts_with('/') { format!("file://{path}") } else { format!("file:///{path}") }
}

fn markdown_link_label(label: &str) -> String {
    label.replace('\\', "\\\\").replace('[', "\\[").replace(']', "\\]")
}

fn markdown_link_destination(destination: &str) -> String {
    destination.replace('>', "%3E")
}

fn max_backtick_run(contents: &str) -> usize {
    let mut max_run = 0usize;
    let mut current = 0usize;
    for ch in contents.chars() {
        if ch == '`' {
            current += 1;
            max_run = max_run.max(current);
        } else {
            current = 0;
        }
    }
    max_run
}
