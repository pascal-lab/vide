import argparse
import re
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

RUST_KEYWORDS = {
    "as",
    "break",
    "const",
    "continue",
    "crate",
    "else",
    "enum",
    "extern",
    "false",
    "fn",
    "for",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "ref",
    "return",
    "self",
    "Self",
    "static",
    "struct",
    "super",
    "trait",
    "true",
    "type",
    "unsafe",
    "use",
    "where",
    "while",
    "async",
    "await",
    "dyn",
    "abstract",
    "become",
    "box",
    "do",
    "final",
    "macro",
    "override",
    "priv",
    "typeof",
    "unsized",
    "virtual",
    "yield",
    "try",
}


@dataclass
class Member:
    kind: str
    ty: str | None
    name: str


@dataclass
class TypeInfo:
    base: str = "SyntaxNode"
    is_final: bool = True
    multi_kind: bool = False
    members: list[Member] = field(default_factory=list)
    combined: list[Member] = field(default_factory=list)


def screaming_snake(name: str) -> str:
    parts = re.findall(r"[A-Z]+(?=[A-Z][a-z]|\d|$)|[A-Z]?[a-z]+|\d+", name)
    return "_".join(part.upper() for part in parts)


def snake(name: str) -> str:
    parts = re.findall(r"[A-Z]+(?=[A-Z][a-z]|\d|$)|[A-Z]?[a-z]+|\d+", name)
    value = "_".join(part.lower() for part in parts)
    return f"r#{value}" if value in RUST_KEYWORDS else value


def method_name(name: str) -> str:
    value = snake(name)
    return value.removeprefix("r#")


def parse_member(raw_ty: str, name: str) -> Member:
    if raw_ty == "token":
        return Member("token", None, name)
    if raw_ty == "tokenlist":
        return Member("tokenlist", None, name)
    if raw_ty.startswith("list<"):
        return Member("list", raw_ty[len("list<") : raw_ty.index(">")], name)
    if raw_ty.startswith("separated_list<"):
        return Member(
            "separated_list", raw_ty[len("separated_list<") : raw_ty.index(">")], name
        )
    if raw_ty.endswith("?"):
        return Member("optional_node", raw_ty[:-1], name)
    return Member("node", raw_ty, name)


def load_types(path: Path) -> tuple[dict[str, TypeInfo], dict[str, str]]:
    all_types: dict[str, TypeInfo] = {"SyntaxNode": TypeInfo()}
    kind_map: dict[str, str] = {}
    current_type: str | None = None
    current_tags: dict[str, str] = {}
    current_members: list[Member] = []
    current_kind_base: str | None = None

    def finish_type() -> None:
        nonlocal current_type, current_tags, current_members
        if current_type is None:
            return

        base = current_tags.get("base", "SyntaxNode")
        is_final = current_tags.get("final", "true") != "false"
        multi_kind = current_tags.get("multiKind") == "true"
        inherited = [] if base == "SyntaxNode" else all_types[base].combined
        all_types[current_type] = TypeInfo(
            base=base,
            is_final=is_final,
            multi_kind=multi_kind,
            members=current_members,
            combined=[*inherited, *current_members],
        )
        if is_final and not multi_kind:
            kind_map[current_type] = current_type

        current_type = None
        current_tags = {}
        current_members = []

    for raw_line in path.read_text().splitlines():
        line = raw_line.strip()
        if line.startswith("//"):
            continue
        if not line or line == "empty":
            current_kind_base = None
            finish_type()
            continue

        if current_kind_base is not None:
            for kind in line.split():
                kind_map[kind] = current_kind_base
            continue

        if current_type is not None:
            raw_ty, member_name = line.split()
            current_members.append(parse_member(raw_ty, member_name))
            continue

        if line.startswith("kindmap<"):
            current_kind_base = line[len("kindmap<") : line.index(">")]
            continue

        parts = line.split()
        current_type = parts[0]
        current_tags = dict(part.split("=", 1) for part in parts[1:])
        current_members = []

    finish_type()
    return all_types, kind_map


def reverse_maps(
    all_types: dict[str, TypeInfo], kind_map: dict[str, str]
) -> tuple[dict[str, set[str]], dict[str, set[str]]]:
    reverse: dict[str, set[str]] = defaultdict(set)
    bases: dict[str, set[str]] = defaultdict(set)
    for kind, ty in kind_map.items():
        reverse[ty].add(kind)
        bases[ty].add(kind)

    for ty_name, ty_info in all_types.items():
        if ty_name == "SyntaxNode" or not ty_info.is_final:
            continue
        current = ty_name
        info = ty_info
        while info.base != "SyntaxNode":
            reverse[info.base].update(reverse[current])
            bases[info.base].add(current)
            current = info.base
            info = all_types[current]
    return reverse, bases


def rust_type_name(ty: str | None) -> str:
    if ty is None:
        raise RuntimeError("missing type")
    return "HybridNode" if ty == "SyntaxNode" else ty


def is_list_member(member: Member) -> bool:
    return member.kind in {"tokenlist", "list", "separated_list"}


def render_prefix(info: TypeInfo, upto: int) -> tuple[list[str], int]:
    lines: list[str] = []
    list_ordinal = 0
    for member in info.combined[:upto]:
        if (
            member.kind == "optional_node"
            or member.kind == "token"
            or member.kind == "node"
        ):
            lines.append("        index += 1;")
        elif is_list_member(member):
            lines.append(
                f"        index += self.syntax().list_child_size({list_ordinal}).unwrap_or(0);"
            )
            list_ordinal += 1
        else:
            raise RuntimeError(f"unknown member kind {member.kind}")
    return lines, list_ordinal


def render_member(info: TypeInfo, index: int, member: Member) -> str:
    name = snake(member.name)
    prefix, list_ordinal = render_prefix(info, index)
    prefix_text = "\n".join(prefix)
    index_decl = "let mut index = 0;" if prefix else "let index = 0;"
    if member.kind == "token":
        return f"""    #[inline]
    pub fn {name}(&self) -> Option<SyntaxToken<'a>> {{
        {index_decl}
{prefix_text}
        self.syntax().child_token(index)
    }}
"""
    if member.kind == "tokenlist":
        return f"""    #[inline]
    pub fn {name}(&self) -> TokenList<'a> {{
        {index_decl}
{prefix_text}
        TokenList::new(self.syntax(), index, self.syntax().list_child_size({list_ordinal}).unwrap_or(0))
    }}
"""
    if member.kind == "list":
        ty = rust_type_name(member.ty)
        return f"""    #[inline]
    pub fn {name}(&self) -> SyntaxList<'a, {ty}<'a>> {{
        {index_decl}
{prefix_text}
        SyntaxList::new(self.syntax(), index, self.syntax().list_child_size({list_ordinal}).unwrap_or(0))
    }}
"""
    if member.kind == "separated_list":
        ty = rust_type_name(member.ty)
        return f"""    #[inline]
    pub fn {name}(&self) -> SeparatedList<'a, {ty}<'a>> {{
        {index_decl}
{prefix_text}
        SeparatedList::new(self.syntax(), index, self.syntax().list_child_size({list_ordinal}).unwrap_or(0))
    }}
"""
    ty = rust_type_name(member.ty)
    if member.kind == "node":
        return f"""    #[inline]
    pub fn {name}(&self) -> {ty}<'a> {{
        {index_decl}
{prefix_text}
        self.syntax().child_node(index).and_then({ty}::cast).unwrap()
    }}
"""
    if member.kind == "optional_node":
        return f"""    #[inline]
    pub fn {name}(&self) -> Option<{ty}<'a>> {{
        {index_decl}
{prefix_text}
        self.syntax().child_node(index).and_then({ty}::cast)
    }}
"""
    raise RuntimeError(f"unknown member kind {member.kind}")


def render_members(info: TypeInfo) -> str:
    return "\n".join(
        render_member(info, index, member) for index, member in enumerate(info.combined)
    )


def render_struct(ty: str, kind: str, members: str) -> str:
    kind_name = screaming_snake(kind)
    return f"""#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct {ty}<'a> {{
    syntax: SyntaxNode<'a>,
}}

impl<'a> {ty}<'a> {{
{members}}}

impl<'a> AstNode<'a> for {ty}<'a> {{
    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {{
        kind == SyntaxKind::{kind_name}
    }}

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {{
        Self::can_cast(syntax.kind()).then_some(Self {{ syntax }})
    }}

    #[inline]
    fn syntax(self) -> SyntaxNode<'a> {{
        self.syntax
    }}
}}
"""


def render_enum(
    ty: str,
    all_types: dict[str, TypeInfo],
    reverse: dict[str, set[str]],
    members: str,
) -> str:
    kinds = sorted(reverse[ty])
    variant_defs = []
    syntax_arms = []
    as_fns = []
    cast_arms = []
    for kind in kinds:
        variant = kind
        payload = variant if variant in all_types and variant != ty else "SyntaxNode"
        variant_defs.append(f"    {variant}({payload}<'a>),")
        syntax_arms.append(
            f"            Self::{variant}(node) => node.syntax(),"
            if payload != "SyntaxNode"
            else f"            Self::{variant}(node) => node,"
        )
        as_fns.append(
            f"""    #[inline]
    pub fn as_{method_name(variant)}(self) -> Option<{payload}<'a>> {{
        match self {{
            Self::{variant}(node) => Some(node),
            _ => None,
        }}
    }}
"""
        )
        if payload != "SyntaxNode":
            cast_arms.append(
                f"            SyntaxKind::{screaming_snake(kind)} => Some(Self::{variant}({variant}::cast(syntax)?)),"
            )
        else:
            cast_arms.append(
                f"            SyntaxKind::{screaming_snake(kind)} => Some(Self::{variant}(syntax)),"
            )

    can_cast = " ||\n            ".join(
        f"kind == SyntaxKind::{screaming_snake(kind)}" for kind in kinds
    )

    return f"""#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum {ty}<'a> {{
{chr(10).join(variant_defs)}
}}

impl<'a> {ty}<'a> {{
{members}
{chr(10).join(as_fns)}}}

impl<'a> AstNode<'a> for {ty}<'a> {{
    #[inline]
    fn can_cast(kind: SyntaxKind) -> bool {{
        {can_cast}
    }}

    #[inline]
    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {{
        match syntax.kind() {{
{chr(10).join(cast_arms)}
            _ => None,
        }}
    }}

    #[inline]
    fn syntax(self) -> SyntaxNode<'a> {{
        match self {{
{chr(10).join(syntax_arms)}
        }}
    }}
}}
"""


def render_ast(all_types: dict[str, TypeInfo], kind_map: dict[str, str]) -> str:
    reverse, _bases = reverse_maps(all_types, kind_map)
    parts = [
        "// This file is generated by crates/slang-sys/scripts/generate_ast.py.",
        "// Do not edit by hand.",
        "",
        "use std::marker::PhantomData;",
        "",
        "use super::super::syntax_node::{SyntaxNode, SyntaxToken};",
        "use super::super::syntax_kind::SyntaxKind;",
        "",
        "pub trait AstNode<'a>: Copy + Clone {",
        "    fn can_cast(kind: SyntaxKind) -> bool where Self: Sized;",
        "    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> where Self: Sized;",
        "    fn syntax(self) -> SyntaxNode<'a>;",
        "}",
        "",
        "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]",
        "pub struct HybridNode<'a> {",
        "    syntax: SyntaxNode<'a>,",
        "}",
        "",
        "/// This is a typed AST node that is converted from `SyntaxNode`.",
        "impl<'a> AstNode<'a> for HybridNode<'a> {",
        "    fn can_cast(_: SyntaxKind) -> bool { true }",
        "    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> { Some(Self { syntax }) }",
        "    fn syntax(self) -> SyntaxNode<'a> { self.syntax }",
        "}",
        "",
        "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]",
        "pub struct TokenList<'a> {",
        "    parent: SyntaxNode<'a>,",
        "    start: usize,",
        "    len: usize,",
        "}",
        "",
        "impl<'a> TokenList<'a> {",
        "    fn new(parent: SyntaxNode<'a>, start: usize, len: usize) -> Self {",
        "        Self { parent, start, len }",
        "    }",
        "",
        "    pub fn children(self) -> impl Iterator<Item = SyntaxToken<'a>> {",
        "        (self.start..self.start + self.len).filter_map(move |index| self.parent.child_token(index))",
        "    }",
        "}",
        "",
        "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]",
        "pub struct SyntaxList<'a, T: AstNode<'a>> {",
        "    parent: SyntaxNode<'a>,",
        "    start: usize,",
        "    len: usize,",
        "    _marker: PhantomData<T>,",
        "}",
        "",
        "impl<'a, T: AstNode<'a>> SyntaxList<'a, T> {",
        "    fn new(parent: SyntaxNode<'a>, start: usize, len: usize) -> Self {",
        "        Self { parent, start, len, _marker: PhantomData }",
        "    }",
        "",
        "    pub fn children(self) -> impl Iterator<Item = T> {",
        "        (self.start..self.start + self.len).filter_map(move |index| self.parent.child_node(index)).filter_map(T::cast)",
        "    }",
        "}",
        "",
        "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]",
        "pub struct SeparatedList<'a, T: AstNode<'a>> {",
        "    parent: SyntaxNode<'a>,",
        "    start: usize,",
        "    len: usize,",
        "    _marker: PhantomData<T>,",
        "}",
        "",
        "impl<'a, T: AstNode<'a>> SeparatedList<'a, T> {",
        "    fn new(parent: SyntaxNode<'a>, start: usize, len: usize) -> Self {",
        "        Self { parent, start, len, _marker: PhantomData }",
        "    }",
        "",
        "    pub fn children(self) -> impl Iterator<Item = T> {",
        "        (self.start..self.start + self.len).step_by(2).filter_map(move |index| self.parent.child_node(index)).filter_map(T::cast)",
        "    }",
        "}",
        "",
    ]

    for ty in sorted(name for name in all_types if name != "SyntaxNode"):
        members = render_members(all_types[ty])
        kinds = reverse.get(ty, set())
        if len(kinds) == 1:
            parts.append(render_struct(ty, next(iter(kinds)), members))
        else:
            parts.append(render_enum(ty, all_types, reverse, members))
    return "\n".join(parts)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Generate Rust AST wrappers from slang syntax.txt"
    )
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    all_types, kind_map = load_types(args.input)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(render_ast(all_types, kind_map))


if __name__ == "__main__":
    main()
