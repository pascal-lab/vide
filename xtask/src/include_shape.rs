//! Classify `` `include `` targets as MacrosOnly / Balanced / Unbalanced.
//!
//! Port of `scripts/include_shape.py`. Error direction is conservative:
//! anything that cannot be proved balanced is `Unbalanced`. Never classify
//! an unbalanced file as `Balanced`.

use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use regex::Regex;

const EXTS: &[&str] = &["sv", "v", "svh", "vh", "svi", "inc", "h", "vi"];

const OPENERS: &[&str] = &[
    "module",
    "macromodule",
    "class",
    "package",
    "interface",
    "program",
    "function",
    "task",
    "generate",
    "checker",
    "property",
    "sequence",
    "covergroup",
    "clocking",
    "config",
    "primitive",
    "specify",
    "table",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IncludeShape {
    MacrosOnly,
    Balanced,
    Unbalanced,
    Unresolved,
    Unreadable,
}

impl IncludeShape {
    fn label(self) -> &'static str {
        match self {
            Self::MacrosOnly => "MacrosOnly",
            Self::Balanced => "Balanced",
            Self::Unbalanced => "Unbalanced",
            Self::Unresolved => "Unresolved",
            Self::Unreadable => "unreadable",
        }
    }
}

#[derive(Debug, Default)]
pub struct ShapeReport {
    pub root: PathBuf,
    pub file_count: usize,
    pub distinct_targets: usize,
    pub include_sites: usize,
    pub unresolved_targets: usize,
    pub unresolved_sites: usize,
    pub shape_files: BTreeMap<IncludeShape, usize>,
    pub shape_sites: BTreeMap<IncludeShape, usize>,
    pub top_included: Vec<(usize, IncludeShape, String, usize)>,
}

impl ShapeReport {
    pub fn site_pct(&self, shape: IncludeShape) -> f64 {
        if self.include_sites == 0 {
            return 0.0;
        }
        let n = *self.shape_sites.get(&shape).unwrap_or(&0);
        100.0 * n as f64 / self.include_sites as f64
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "corpus: {}", self.root.display());
        let _ = writeln!(out, "total SV files: {}", self.file_count);
        let _ = writeln!(
            out,
            "distinct include targets: {}   total include sites: {}",
            self.distinct_targets, self.include_sites
        );
        let _ = writeln!(
            out,
            "unresolved targets: {} ({} sites)",
            self.unresolved_targets, self.unresolved_sites
        );
        out.push('\n');

        let _ = writeln!(out, "=== by distinct included file ===");
        let tot: usize = self.shape_files.values().sum();
        let mut files: Vec<_> = self.shape_files.iter().collect();
        files.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (shape, count) in files {
            let pct = if tot == 0 { 0.0 } else { 100.0 * *count as f64 / tot as f64 };
            let _ = writeln!(out, "  {:12} {:5}  {:5.1}%", shape.label(), count, pct);
        }

        let _ = writeln!(
            out,
            "\n=== weighted by include sites (this is what matters for invalidation) ==="
        );
        let tot: usize = self.shape_sites.values().sum();
        let mut sites: Vec<_> = self.shape_sites.iter().collect();
        sites.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (shape, count) in sites {
            let pct = if tot == 0 { 0.0 } else { 100.0 * *count as f64 / tot as f64 };
            let _ = writeln!(out, "  {:12} {:6}  {:5.1}%", shape.label(), count, pct);
        }

        let unbalanced = self.site_pct(IncludeShape::Unbalanced);
        let _ = writeln!(
            out,
            "\nT8 gate (site-weighted Unbalanced): {unbalanced:.1}%  {}",
            if unbalanced <= 5.0 {
                "<= 5% — T8 may proceed later"
            } else {
                "> 5% — T8 must be redesigned, not silently skipped"
            }
        );

        let _ = writeln!(out, "\n=== top 25 most-included files ===");
        for (nsites, shape, target, size) in self.top_included.iter().take(25) {
            let _ = writeln!(
                out,
                "  {nsites:5} sites  {:11}  {target}  (residue tokens: {size})",
                shape.label()
            );
        }
        out
    }
}

pub fn classify_corpus(roots: &[PathBuf]) -> Result<ShapeReport> {
    if roots.is_empty() {
        bail!("at least one corpus directory is required");
    }
    for root in roots {
        if !root.is_dir() {
            bail!("corpus is not a directory: {}", root.display());
        }
    }

    let files: Vec<PathBuf> = roots.iter().flat_map(|root| collect_sv_files(root)).collect();
    let mut by_name: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    for path in &files {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            by_name.entry(name.to_owned()).or_default().push(path.clone());
        }
    }

    let include_re = include_regex();
    let mut edges: BTreeMap<String, usize> = BTreeMap::new();
    let mut unresolved: BTreeMap<String, usize> = BTreeMap::new();
    for path in &files {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        for cap in include_re.captures_iter(&text) {
            let Some(target) = cap.get(1).map(|m| Path::new(m.as_str())) else {
                continue;
            };
            let Some(name) = target.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            *edges.entry(name.to_owned()).or_default() += 1;
            if !by_name.contains_key(name) {
                *unresolved.entry(name.to_owned()).or_default() += 1;
            }
        }
    }

    let mut report = ShapeReport {
        root: if roots.len() == 1 {
            roots[0].clone()
        } else {
            PathBuf::from(
                roots.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("+"),
            )
        },
        file_count: files.len(),
        distinct_targets: edges.len(),
        include_sites: edges.values().sum(),
        unresolved_targets: unresolved.len(),
        unresolved_sites: unresolved.values().sum(),
        ..ShapeReport::default()
    };

    let mut detail = Vec::new();
    for (target, nsites) in &edges {
        let Some(cands) = by_name.get(target) else {
            *report.shape_files.entry(IncludeShape::Unresolved).or_default() += 1;
            *report.shape_sites.entry(IncludeShape::Unresolved).or_default() += nsites;
            continue;
        };
        let (shape, size) = classify_path(&cands[0]);
        *report.shape_files.entry(shape).or_default() += 1;
        *report.shape_sites.entry(shape).or_default() += nsites;
        detail.push((*nsites, shape, target.clone(), size));
    }
    detail.sort_by(|a, b| b.0.cmp(&a.0).then(a.2.cmp(&b.2)));
    report.top_included = detail;
    Ok(report)
}

pub fn run(roots: &[PathBuf]) -> Result<()> {
    let report = classify_corpus(roots).with_context(|| {
        format!(
            "classify {}",
            roots.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(" ")
        )
    })?;
    print!("{}", report.render());
    Ok(())
}

fn collect_sv_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn rec(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        let mut ents: Vec<_> = entries.flatten().collect();
        ents.sort_by_key(|e| e.file_name());
        for ent in ents {
            let path = ent.path();
            if path.is_dir() {
                if path.file_name().and_then(|n| n.to_str()) == Some(".git") {
                    continue;
                }
                rec(&path, out);
            } else if is_sv(&path) {
                out.push(path);
            }
        }
    }
    rec(root, &mut out);
    out
}

fn is_sv(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|ext| EXTS.contains(&ext))
}

fn include_regex() -> Regex {
    Regex::new(r#"(?m)^\s*`include\s+[<"]([^">]+)[">]"#).expect("static include regex")
}

fn directive_regex() -> Regex {
    Regex::new(
        r"^\s*`(define|ifdef|ifndef|elsif|else|endif|undef|include|timescale|default_nettype|line|pragma|celldefine|endcelldefine|resetall|unconnected_drive|nounconnected_drive|begin_keywords|end_keywords)\b",
    )
    .expect("static directive regex")
}

fn ident_regex() -> Regex {
    Regex::new(r"\b[A-Za-z_][A-Za-z0-9_$]*\b").expect("static ident regex")
}

fn closer_for(opener: &str) -> String {
    // Same table as scripts/include_shape.py: `end` + opener, with the
    // three SV exceptions. `covergroup` therefore pairs with
    // `endcovergroup`, not `endgroup` — keep the lexical approximation
    // conservative rather than "more correct".
    match opener {
        "generate" => "endgenerate".to_owned(),
        "specify" => "endspecify".to_owned(),
        "table" => "endtable".to_owned(),
        other => format!("end{other}"),
    }
}

pub fn classify_source(raw: &str) -> (IncludeShape, usize) {
    let body = strip_macro_bodies(&strip_comments(raw));
    let residue = body.trim();
    if residue.is_empty() {
        return (IncludeShape::MacrosOnly, 0);
    }
    let ident_re = ident_regex();
    let toks: Vec<&str> = ident_re.find_iter(residue).map(|m| m.as_str()).collect();
    if toks.is_empty() {
        return (IncludeShape::MacrosOnly, 0);
    }

    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for tok in &toks {
        *counts.entry(*tok).or_default() += 1;
    }
    let mut imbalance = 0usize;
    for op in OPENERS {
        let closer = closer_for(op);
        let open = *counts.get(op).unwrap_or(&0);
        let close = *counts.get(closer.as_str()).unwrap_or(&0);
        imbalance += open.abs_diff(close);
    }
    for (a, b) in [('(', ')'), ('[', ']'), ('{', '}')] {
        imbalance += residue
            .chars()
            .filter(|&c| c == a)
            .count()
            .abs_diff(residue.chars().filter(|&c| c == b).count());
    }
    if imbalance == 0 {
        (IncludeShape::Balanced, toks.len())
    } else {
        (IncludeShape::Unbalanced, toks.len())
    }
}

fn classify_path(path: &Path) -> (IncludeShape, usize) {
    match fs::read_to_string(path) {
        Ok(raw) => classify_source(&raw),
        Err(_) => (IncludeShape::Unreadable, 0),
    }
}

fn strip_comments(s: &str) -> String {
    let block = Regex::new(r"(?s)/\*.*?\*/").expect("block comment regex");
    let without_block = block.replace_all(s, " ");
    let line = Regex::new(r"//[^\n]*").expect("line comment regex");
    line.replace_all(&without_block, " ").into_owned()
}

fn strip_macro_bodies(s: &str) -> String {
    let directive = directive_regex();
    let mut out = Vec::new();
    let lines: Vec<&str> = s.split('\n').collect();
    let mut i = 0;
    while i < lines.len() {
        let mut line = lines[i];
        if directive.is_match(line) {
            while line.trim_end().ends_with('\\') && i + 1 < lines.len() {
                i += 1;
                line = lines[i];
            }
            i += 1;
            continue;
        }
        out.push(line);
        i += 1;
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_defines_are_macros_only() {
        let src = "`define WIDTH 8\n`define DEPTH 4\n";
        assert_eq!(classify_source(src).0, IncludeShape::MacrosOnly);
    }

    #[test]
    fn a_closed_class_is_balanced() {
        let src = "class foo extends uvm_object;\n  `uvm_object_utils(foo)\nendclass\n";
        assert_eq!(classify_source(src).0, IncludeShape::Balanced);
    }

    #[test]
    fn an_unclosed_module_is_unbalanced_not_balanced() {
        let src = "module foo;\n  wire x;\n";
        assert_eq!(classify_source(src).0, IncludeShape::Unbalanced);
        assert_ne!(classify_source(src).0, IncludeShape::Balanced);
    }

    #[test]
    fn unmatched_paren_is_unbalanced() {
        let src = "function int f;\n  return (1;\nendfunction\n";
        assert_eq!(classify_source(src).0, IncludeShape::Unbalanced);
    }

    #[test]
    fn classify_never_promotes_unbalanced_to_balanced() {
        // Conservative direction: we may call a balanced file Unbalanced,
        // but never the reverse. This source opens a class and a module
        // and closes neither.
        let src = "class c;\nmodule m;\n";
        assert_eq!(classify_source(src).0, IncludeShape::Unbalanced);
    }
}
