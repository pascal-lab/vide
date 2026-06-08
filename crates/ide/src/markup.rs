use std::fmt;

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

    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }

    pub fn newline(&mut self) {
        self.text.push_str("\n\n");
    }

    pub fn horizontal_line(&mut self) {
        self.text.push_str("\n\n---------\n\n");
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
        let delimiter_len = max_backtick_run(contents).saturating_add(1);
        let delimiter = "`".repeat(delimiter_len);
        self.text.push_str(&delimiter);
        if contents.contains('`') {
            self.text.push(' ');
        }
        self.text.push_str(contents);
        if contents.contains('`') {
            self.text.push(' ');
        }
        self.text.push_str(&delimiter);
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
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
