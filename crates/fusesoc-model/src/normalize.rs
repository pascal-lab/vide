//! Normalization of raw CAPI2 model: `*_append` merging and file attribute
//! inheritance.
//!
//! After normalization, each [`Fileset`] has a single `files` list and a
//! single `depend` list, and each file entry carries effective attributes
//! (inheriting defaults from its fileset where not overridden).

use crate::raw::{Core, FileEntry, Fileset, Target};

/// Normalize a [`Core`] in place: merge `*_append` fields and resolve file
/// attribute inheritance.
pub fn normalize_core(core: &mut Core) {
    for fileset in core.filesets.values_mut() {
        normalize_fileset(fileset);
    }
    for target in core.targets.values_mut() {
        normalize_target(target);
    }
}

/// Merge `*_append` into the base list for a fileset.
fn normalize_fileset(fs: &mut Fileset) {
    // Merge files_append into files.
    if !fs.files_append.is_empty() {
        fs.files.extend(fs.files_append.drain(..));
        fs.files_append.clear();
    }
    // Merge depend_append into depend.
    if !fs.depend_append.is_empty() {
        fs.depend.extend(fs.depend_append.drain(..));
        fs.depend_append.clear();
    }

    // Resolve file attribute inheritance: file-level overrides fileset defaults.
    for entry in &mut fs.files {
        if let FileEntry::WithAttributes(map) = entry {
            if let Some((_path, attrs)) = map.iter_mut().next() {
                // Inherit file_type from fileset if not set on file.
                if attrs.file_type.is_none() {
                    attrs.file_type = fs.file_type.clone();
                }
                // Inherit logical_name from fileset if not set on file.
                if attrs.logical_name.is_none() {
                    attrs.logical_name = fs.logical_name.clone();
                }
                // Append fileset tags to file tags (file tags come first per
                // FuseSoC spec: "Appends the tags set on the containing fileset").
                if !fs.tags.is_empty() {
                    let mut combined = attrs.tags.clone();
                    combined.extend(fs.tags.iter().cloned());
                    attrs.tags = combined;
                }
            }
        }
    }
}

/// Merge `*_append` for a target.
fn normalize_target(target: &mut Target) {
    if !target.filesets_append.is_empty() {
        target.filesets.extend(target.filesets_append.drain(..));
        target.filesets_append.clear();
    }
}

/// Get the effective file type for a file entry, falling back to the fileset
/// default.
pub fn effective_file_type(entry: &FileEntry, fs: &Fileset) -> Option<String> {
    entry
        .attributes()
        .and_then(|a| a.file_type.clone())
        .or_else(|| fs.file_type.clone())
}

/// Get the effective include path for a file entry.
///
/// If `include_path` is set on the file, use it.  Otherwise, if the file is an
/// include file, use the directory containing the file.
pub fn effective_include_path(
    entry: &FileEntry,
    _core_root: &utils::paths::AbsPath,
) -> Option<String> {
    let attrs = entry.attributes()?;
    if let Some(ip) = &attrs.include_path {
        return Some(ip.clone());
    }
    if attrs.is_include_file {
        // Use the directory containing the file.
        let path = entry.path();
        return path.rsplit_once('/').map(|(dir, _)| dir.to_string());
    }
    None
}

/// Get the effective defines for a file entry.
pub fn effective_defines(entry: &FileEntry) -> Vec<(String, String)> {
    let Some(attrs) = entry.attributes() else {
        return Vec::new();
    };
    let Some(defs) = &attrs.define else {
        return Vec::new();
    };
    defs.iter()
        .map(|(k, v)| (k.clone(), format_define_value(v)))
        .collect()
}

fn format_define_value(v: &crate::raw::FileDefineValue) -> String {
    match v {
        crate::raw::FileDefineValue::Str(s) => s.clone(),
        crate::raw::FileDefineValue::Int(i) => i.to_string(),
        crate::raw::FileDefineValue::Bool(b) => b.to_string(),
    }
}

/// Check if a file type is SystemVerilog or Verilog (processable by Vide).
/// Check if a file type is Verilog or SystemVerilog (processable by Vide).
pub fn is_verilog_file_type(file_type: &str) -> bool {
    let ft = file_type.to_ascii_lowercase();
    ft.contains("verilog")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::{FileAttributes, FileEntry};
    use indexmap::indexmap;

    #[test]
    fn merges_files_append() {
        let mut fs = Fileset {
            file_type: Some("systemVerilogSource".into()),
            logical_name: None,
            tags: vec![],
            files: vec![FileEntry::Path("a.sv".into())],
            files_append: vec![FileEntry::Path("b.sv".into())],
            depend: vec![],
            depend_append: vec![],
        };
        normalize_fileset(&mut fs);
        assert_eq!(fs.files.len(), 2);
        assert!(fs.files_append.is_empty());
    }

    #[test]
    fn merges_depend_append() {
        let mut fs = Fileset {
            file_type: None,
            logical_name: None,
            tags: vec![],
            files: vec![],
            files_append: vec![],
            depend: vec!["base".into()],
            depend_append: vec!["extra".into()],
        };
        normalize_fileset(&mut fs);
        assert_eq!(fs.depend, vec!["base", "extra"]);
        assert!(fs.depend_append.is_empty());
    }

    #[test]
    fn inherits_file_type() {
        let mut fs = Fileset {
            file_type: Some("verilogSource".into()),
            logical_name: None,
            tags: vec![],
            files: vec![FileEntry::WithAttributes(indexmap! {
                "rtl/top.v".to_string() => FileAttributes::default(),
            })],
            files_append: vec![],
            depend: vec![],
            depend_append: vec![],
        };
        normalize_fileset(&mut fs);
        if let FileEntry::WithAttributes(map) = &fs.files[0] {
            let attrs = map.values().next().unwrap();
            assert_eq!(attrs.file_type.as_deref(), Some("verilogSource"));
        } else {
            panic!("expected WithAttributes");
        }
    }

    #[test]
    fn file_overrides_fileset_file_type() {
        let mut fs = Fileset {
            file_type: Some("verilogSource".into()),
            logical_name: None,
            tags: vec![],
            files: vec![FileEntry::WithAttributes(indexmap! {
                "rtl/top.sv".to_string() => FileAttributes {
                    file_type: Some("systemVerilogSource".into()),
                    ..Default::default()
                },
            })],
            files_append: vec![],
            depend: vec![],
            depend_append: vec![],
        };
        normalize_fileset(&mut fs);
        if let FileEntry::WithAttributes(map) = &fs.files[0] {
            let attrs = map.values().next().unwrap();
            assert_eq!(attrs.file_type.as_deref(), Some("systemVerilogSource"));
        } else {
            panic!("expected WithAttributes");
        }
    }
}