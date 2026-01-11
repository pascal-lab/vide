use ide_db::root_db::RootDb;
use span::FilePosition;
use utils::text_edit::TextEditItem;

use super::named::{CompletionItem, CompletionItemKind};
use crate::completion::context::{CompletionContext, SynContext};

pub(super) fn complete_keywords(
    _db: &RootDb,
    _position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    if !matches!(ctx.syn, SynContext::TopLevel | SynContext::ModuleHeader | SynContext::ModuleItem)
    {
        return Vec::new();
    }

    let all = match ctx.syn {
        SynContext::TopLevel => top_level_keywords(),
        SynContext::ModuleHeader => module_header_keywords(),
        SynContext::ModuleItem => module_item_keywords(),
        _ => &[],
    };

    all.iter()
        .filter(|kw| kw.label.starts_with(prefix))
        .map(|kw| kw.to_completion(ctx.replacement))
        .collect()
}

#[derive(Clone, Copy)]
struct Keyword<'a> {
    label: &'a str,
    plain: &'a str,
    snippet: Option<&'a str>,
    kind: CompletionItemKind,
}

impl Keyword<'_> {
    fn to_completion(&self, replace: utils::text_edit::TextRange) -> CompletionItem {
        CompletionItem {
            label: self.label.to_string(),
            kind: self.kind,
            edit: Some(TextEditItem::replace(replace, self.plain.to_string())),
            snippet_edit: self.snippet.map(|s| TextEditItem::replace(replace, s.to_string())),
        }
    }
}

fn top_level_keywords() -> &'static [Keyword<'static>] {
    &[Keyword {
        label: "module",
        plain: "module",
        snippet: Some("module ${1:name} (${2:ports});\n\t${0}\nendmodule"),
        kind: CompletionItemKind::Snippet,
    }]
}

fn module_header_keywords() -> &'static [Keyword<'static>] {
    &[
        Keyword {
            label: "input",
            plain: "input",
            snippet: None,
            kind: CompletionItemKind::Keyword,
        },
        Keyword {
            label: "output",
            plain: "output",
            snippet: None,
            kind: CompletionItemKind::Keyword,
        },
        Keyword {
            label: "inout",
            plain: "inout",
            snippet: None,
            kind: CompletionItemKind::Keyword,
        },
        Keyword {
            label: "parameter",
            plain: "parameter",
            snippet: None,
            kind: CompletionItemKind::Keyword,
        },
        Keyword {
            label: "localparam",
            plain: "localparam",
            snippet: None,
            kind: CompletionItemKind::Keyword,
        },
    ]
}

fn module_item_keywords() -> &'static [Keyword<'static>] {
    &[
        Keyword {
            label: "wire",
            plain: "wire",
            snippet: Some("wire ${1:name};"),
            kind: CompletionItemKind::Snippet,
        },
        Keyword {
            label: "reg",
            plain: "reg",
            snippet: Some("reg ${1:name};"),
            kind: CompletionItemKind::Snippet,
        },
        Keyword {
            label: "assign",
            plain: "assign",
            snippet: Some("assign ${1:lhs} = ${0:rhs};"),
            kind: CompletionItemKind::Snippet,
        },
        Keyword {
            label: "always",
            plain: "always",
            snippet: Some("always @(${1:*}) begin\n\t${0}\nend"),
            kind: CompletionItemKind::Snippet,
        },
        Keyword {
            label: "always @(*)",
            plain: "always",
            snippet: Some("always @(*) begin\n\t${0}\nend"),
            kind: CompletionItemKind::Snippet,
        },
        Keyword {
            label: "initial",
            plain: "initial",
            snippet: Some("initial begin\n\t${0}\nend"),
            kind: CompletionItemKind::Snippet,
        },
        Keyword {
            label: "begin",
            plain: "begin",
            snippet: Some("begin\n\t${0}\nend"),
            kind: CompletionItemKind::Snippet,
        },
        Keyword {
            label: "if",
            plain: "if",
            snippet: Some("if (${1:cond}) begin\n\t${0}\nend"),
            kind: CompletionItemKind::Snippet,
        },
        Keyword {
            label: "ifelse",
            plain: "if",
            snippet: Some("if (${1:cond}) begin\n\t${2}\nend else begin\n\t${0}\nend"),
            kind: CompletionItemKind::Snippet,
        },
        Keyword {
            label: "case",
            plain: "case",
            snippet: Some(
                "case (${1:expr})\n\t${2:val}: ${3:stmt};\n\tdefault: ${0:stmt};\nendcase",
            ),
            kind: CompletionItemKind::Snippet,
        },
        Keyword {
            label: "for",
            plain: "for",
            snippet: Some(
                "for (${1:i} = ${2:0}; ${3:i} < ${4:N}; ${5:i} = ${6:i} + 1) begin\n\t${0}\nend",
            ),
            kind: CompletionItemKind::Snippet,
        },
        Keyword {
            label: "while",
            plain: "while",
            snippet: Some("while (${1:cond}) begin\n\t${0}\nend"),
            kind: CompletionItemKind::Snippet,
        },
        Keyword {
            label: "repeat",
            plain: "repeat",
            snippet: Some("repeat (${1:N}) begin\n\t${0}\nend"),
            kind: CompletionItemKind::Snippet,
        },
        Keyword {
            label: "parameter",
            plain: "parameter",
            snippet: None,
            kind: CompletionItemKind::Keyword,
        },
        Keyword {
            label: "localparam",
            plain: "localparam",
            snippet: None,
            kind: CompletionItemKind::Keyword,
        },
    ]
}
