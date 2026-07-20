#!/usr/bin/env python3

import argparse
import json
import re
import shlex
from pathlib import Path


def screaming_snake(name: str) -> str:
    parts = re.findall(r"[A-Z]+(?=[A-Z][a-z]|\d|$)|[A-Z]?[a-z]+|\d+", name)
    return "_".join(part.upper() for part in parts)


def rust_str(value: str) -> str:
    return json.dumps(value)


def parse_subsystems(path: Path) -> list[str]:
    text = path.read_text()
    match = re.search(r"#define DS\(x\) \\\n(?P<body>.*?)\nSLANG_ENUM_SIZED\(DiagSubsystem", text, re.S)
    if not match:
        raise RuntimeError(f"failed to find DiagSubsystem definition in {path}")

    subsystems = []
    for line in match.group("body").splitlines():
        line = line.strip().rstrip("\\").strip()
        if not line:
            continue

        item = re.fullmatch(r"x\((?P<name>[A-Za-z_][A-Za-z0-9_]*)\)", line)
        if not item:
            raise RuntimeError(f"invalid DiagSubsystem entry in {path}: {line}")
        subsystems.append(item.group("name"))

    return subsystems


def parse_diagnostics(path: Path):
    diagnostics_by_subsystem = {}
    diagnostic_names = set()
    option_map = {}
    groups = []
    current_subsystem = "General"
    current_group = None

    def finish_group(parts):
        nonlocal current_group
        if current_group is None:
            return

        for part in parts:
            if part == "}":
                groups.append(current_group)
                current_group = None
                return
            current_group[1].append(part)

    for raw_line in path.read_text().splitlines():
        line = raw_line.strip()
        if not line or line.startswith("//"):
            continue

        parts = shlex.split(line)
        if current_group is not None:
            finish_group(parts)
            continue

        if parts[0] == "subsystem":
            current_subsystem = parts[1]
            if current_subsystem not in diagnostics_by_subsystem:
                diagnostics_by_subsystem[current_subsystem] = []
            continue

        if parts[0] == "group":
            current_group = (parts[1], [])
            if parts[2] != "=" or parts[3] != "{":
                raise RuntimeError(f"invalid group declaration: {line}")
            finish_group(parts[4:])
            continue

        severity_token = parts[0]
        if severity_token == "warning":
            option_name = parts[1]
            name = parts[2]
            message = parts[3]
            severity = "Warning"
        elif severity_token in ("error", "fatal", "note"):
            option_name = ""
            name = parts[1]
            message = parts[2]
            severity = severity_token.capitalize()
        else:
            raise RuntimeError(f"invalid diagnostic entry: {line}")

        if name in diagnostic_names:
            raise RuntimeError(f"duplicate diagnostic name: {name}")
        diagnostic_names.add(name)

        diagnostic = {
            "name": name,
            "subsystem": current_subsystem,
            "severity": severity,
            "message": message,
            "option_name": option_name,
        }
        diagnostics_by_subsystem[current_subsystem].append(diagnostic)
        if option_name:
            option_map.setdefault(option_name, []).append(name)

    diagnostics = []
    for subsystem in sorted(diagnostics_by_subsystem):
        subsystem_diagnostics = sorted(
            diagnostics_by_subsystem[subsystem],
            key=lambda d: (d["severity"], d["name"], d["message"], d["option_name"]),
        )
        for code, diagnostic in enumerate(subsystem_diagnostics):
            diagnostic["code"] = code
            diagnostics.append(diagnostic)

    if current_group is not None:
        raise RuntimeError(f"unterminated diagnostic group: {current_group[0]}")

    return diagnostics_by_subsystem, diagnostics, option_map, groups


def render_subsystem(subsystems: list[str]) -> str:
    variant_defs = [f"    {name} = {index}," for index, name in enumerate(subsystems)]
    debug_arms = [f'            Self::{name} => "{name}",' for name in subsystems]
    from_raw_arms = [f"            {index} => Some(Self::{name})," for index, name in enumerate(subsystems)]

    return f"""#[repr(u16)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagSubsystem {{
{chr(10).join(variant_defs)}
}}

impl DiagSubsystem {{
    #[inline]
    pub const fn from_raw(raw: u16) -> Option<Self> {{
        match raw {{
{chr(10).join(from_raw_arms)}
            _ => None,
        }}
    }}

    #[inline]
    pub const fn as_raw(self) -> u16 {{
        self as u16
    }}
}}

impl fmt::Debug for DiagSubsystem {{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {{
        let name = match *self {{
{chr(10).join(debug_arms)}
        }};
        f.write_str(name)
    }}
}}
"""


def render_severity() -> str:
    variants = ["Ignored", "Note", "Warning", "Error", "Fatal"]
    variant_defs = [f"    {name} = {index}," for index, name in enumerate(variants)]
    debug_arms = [f'            Self::{name} => "{name}",' for name in variants]
    from_raw_arms = [f"            {index} => Some(Self::{name})," for index, name in enumerate(variants)]

    return f"""#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticSeverity {{
{chr(10).join(variant_defs)}
}}

impl DiagnosticSeverity {{
    #[inline]
    pub const fn from_raw(raw: u8) -> Option<Self> {{
        match raw {{
{chr(10).join(from_raw_arms)}
            _ => None,
        }}
    }}

    #[inline]
    pub const fn as_raw(self) -> u8 {{
        self as u8
    }}
}}

impl fmt::Debug for DiagnosticSeverity {{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {{
        let name = match *self {{
{chr(10).join(debug_arms)}
        }};
        f.write_str(name)
    }}
}}
"""


def render_diag_code(subsystems: list[str], diagnostics: list[dict]) -> str:
    subsystem_ids = {name: index for index, name in enumerate(subsystems)}
    constants = []
    debug_arms = []
    all_values = []

    for diagnostic in diagnostics:
        const_name = screaming_snake(diagnostic["name"])
        subsystem = diagnostic["subsystem"]
        constants.append(
            f"    pub const {const_name}: Self = Self {{ subsystem: {subsystem_ids[subsystem]}, code: {diagnostic['code']} }};"
        )
        debug_arms.append(f'            Self::{const_name} => "{diagnostic["name"]}",')
        all_values.append(f"Self::{const_name}")

    return f"""#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiagCode {{
    subsystem: u16,
    code: u16,
}}

impl DiagCode {{
{chr(10).join(constants)}

    pub const ALL: &'static [Self] = &[{", ".join(all_values)}];

    #[inline]
    pub const fn from_raw(subsystem: u16, code: u16) -> Self {{
        Self {{ subsystem, code }}
    }}

    #[inline]
    pub const fn subsystem_raw(self) -> u16 {{
        self.subsystem
    }}

    #[inline]
    pub const fn code_raw(self) -> u16 {{
        self.code
    }}

    #[inline]
    pub const fn subsystem(self) -> Option<DiagSubsystem> {{
        DiagSubsystem::from_raw(self.subsystem)
    }}

    pub fn info(self) -> Option<&'static DiagnosticInfo> {{
        DIAGNOSTIC_INFOS.iter().find(|info| info.code == self)
    }}
}}

impl fmt::Debug for DiagCode {{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {{
        let name = match *self {{
{chr(10).join(debug_arms)}
            _ => return write!(f, "DiagCode({{}}, {{}})", self.subsystem, self.code),
        }};
        f.write_str(name)
    }}
}}
"""


def render_infos(diagnostics: list[dict]) -> str:
    rows = []
    for diagnostic in diagnostics:
        const_name = screaming_snake(diagnostic["name"])
        subsystem = diagnostic["subsystem"]
        option = (
            f"Some({rust_str(diagnostic['option_name'])})"
            if diagnostic["option_name"]
            else "None"
        )
        rows.append(
            "    DiagnosticInfo { "
            f"code: DiagCode::{const_name}, "
            f"name: {rust_str(diagnostic['name'])}, "
            f"subsystem: DiagSubsystem::{subsystem}, "
            f"severity: DiagnosticSeverity::{diagnostic['severity']}, "
            f"default_message: {rust_str(diagnostic['message'])}, "
            f"option_name: {option} "
            "},"
        )

    return f"""#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagnosticInfo {{
    pub code: DiagCode,
    pub name: &'static str,
    pub subsystem: DiagSubsystem,
    pub severity: DiagnosticSeverity,
    pub default_message: &'static str,
    pub option_name: Option<&'static str>,
}}

pub const DIAGNOSTIC_INFOS: &'static [DiagnosticInfo] = &[
{chr(10).join(rows)}
];
"""


def render_groups(groups: list[tuple[str, list[str]]], option_map: dict[str, list[str]]) -> str:
    group_const_defs = []
    group_infos = []

    for name, options in sorted(groups):
        diag_names = []
        for option in sorted(options):
            diag_names.extend(option_map[option])

        const_name = f"{screaming_snake(name)}_GROUP_DIAGNOSTICS"
        values = ", ".join(f"DiagCode::{screaming_snake(diag_name)}" for diag_name in sorted(diag_names))
        group_const_defs.append(f"const {const_name}: &[DiagCode] = &[{values}];")
        group_infos.append(
            f"    DiagnosticGroup {{ name: {rust_str(name)}, diagnostics: {const_name} }},"
        )

    return f"""#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagnosticGroup {{
    pub name: &'static str,
    pub diagnostics: &'static [DiagCode],
}}

{chr(10).join(group_const_defs)}

pub const DIAGNOSTIC_GROUPS: &'static [DiagnosticGroup] = &[
{chr(10).join(group_infos)}
];
"""


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate Rust diagnostic definitions from slang diagnostics.txt")
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--diagnostics-header", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    subsystems = parse_subsystems(args.diagnostics_header)
    diagnostics_by_subsystem, diagnostics, option_map, groups = parse_diagnostics(args.input)
    missing_subsystems = sorted(set(diagnostics_by_subsystem) - set(subsystems))
    if missing_subsystems:
        raise RuntimeError(f"diagnostics.txt uses unknown subsystem(s): {missing_subsystems}")

    output = "\n\n".join(
        [
            "// This file is generated by crates/slang-sys/scripts/generate_diagnostic.py.",
            "// Do not edit by hand.",
            "",
            "use std::fmt;",
            render_subsystem(subsystems),
            render_severity(),
            render_diag_code(subsystems, diagnostics),
            render_infos(diagnostics),
            render_groups(groups, option_map),
            "",
        ]
    )

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(output)


if __name__ == "__main__":
    main()
