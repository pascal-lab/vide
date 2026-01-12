#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path
import re


MACROS = [
    "KEYWORDS_1364_1995",
    "NEWKEYWORDS_1364_2001_noconfig",
    "NEWKEYWORDS_1364_2001",
    "NEWKEYWORDS_1364_2005",
]


def extract_macro(text: str, name: str) -> list[str]:
    start = text.find(f"#define {name}")
    if start < 0:
        raise RuntimeError(f"missing macro {name}")
    end = text.find("\n#define ", start + 1)
    if end < 0:
        end = len(text)
    block = text[start:end]
    return re.findall(r'\{\s*"([^"]+)"\s*,\s*TokenKind::[A-Za-z0-9_]+\s*\}', block)


def main() -> int:
    repo_root = Path(__file__).resolve().parents[1]
    lexer_path = repo_root / "crates" / "slang" / "source" / "parsing" / "LexerFacts.cpp"
    out_path = (
        repo_root
        / "crates"
        / "ide"
        / "src"
        / "completion"
        / "engine"
        / "keywords.generated.toml"
    )

    text = lexer_path.read_text(encoding="utf-8")
    keywords: list[str] = []
    seen: set[str] = set()
    for macro in MACROS:
        for kw in extract_macro(text, macro):
            if kw not in seen:
                seen.add(kw)
                keywords.append(kw)

    out_lines = [
        "# Generated from crates/slang/source/parsing/LexerFacts.cpp",
        "# Verilog-2005 keyword set (1364-1995 + 1364-2001 + 1364-2005).",
    ]
    for kw in keywords:
        out_lines.extend(
            [
                "",
                "[[module_item]]",
                f'label = "{kw}"',
                f'plain = "{kw}"',
                'kind = "keyword"',
            ]
        )

    out_path.write_text("\n".join(out_lines) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
