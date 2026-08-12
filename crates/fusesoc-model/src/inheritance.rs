//! YAML inheritance merge (`<<`) with FuseSoC semantics.
//!
//! FuseSoC replaces the standard YAML merge key (`<<`) with a custom operator
//! and implements its own merge semantics:
//!
//! - Maps are recursively merged.
//! - Lists are replaced by the child's list (NOT concatenated).
//! - Only `_append` lists are concatenated (handled by [`normalize`]).
//!
//! This module replicates that behavior so `.core` files using `<<:` anchors
//! are handled correctly.
//!
//! See: <https://github.com/olofk/fusesoc/blob/main/fusesoc/capi2/inheritance.py>

use serde_yaml_ng::Value;

/// Replace YAML merge key `<<` with a placeholder before deserialization.
///
/// FuseSoC does this via regex on the raw text, then processes the placeholder
/// after YAML parsing.  We take a simpler approach: deserializing into
/// `serde_yaml_ng::Value` already resolves standard YAML merge keys, so we
/// just need to handle the merge result correctly.
///
/// Standard YAML merge (`<<`) already merges maps.  FuseSoC's divergence from
/// standard YAML merge is in how lists are handled: standard merge keeps the
/// child's list, which is actually what FuseSoC does too ( FuseSoC only
/// concatenates `_append` keys).  So for our purposes, the standard YAML merge
/// behavior is sufficient — FuseSoC's custom operator was introduced to work
/// around a PyYAML limitation.
///
/// Therefore this module is currently a no-op passthrough; we rely on the YAML
/// library's built-in merge key support.  This is documented here so future
/// maintainers know the design decision.

/// Merge `parent` into `child` with FuseSoC semantics.
///
/// - For maps: recursively merge keys; child wins on scalar conflicts.
/// - For lists: child replaces parent (FuseSoC does not concatenate plain lists).
/// - For scalars: child replaces parent.
pub fn merge(parent: &Value, child: &Value) -> Value {
    match (parent, child) {
        (Value::Mapping(p), Value::Mapping(c)) => {
            let mut result = p.clone();
            for (key, child_val) in c {
                if let Some(parent_val) = p.get(key) {
                    result.insert(key.clone(), merge(parent_val, child_val));
                } else {
                    result.insert(key.clone(), child_val.clone());
                }
            }
            Value::Mapping(result)
        }
        // Lists and scalars: child wins.
        (_, child) => child.clone(),
    }
}

/// Convenience: merge a list of values in left-to-right order.
///
/// Each subsequent value merges into the accumulated result.
pub fn merge_all(values: &[Value]) -> Value {
    values
        .iter()
        .fold(Value::Null, |acc, v| merge(&acc, v))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml_ng::Value;

    fn yaml(s: &str) -> Value {
        serde_yaml_ng::from_str(s).unwrap()
    }

    #[test]
    fn scalar_child_wins() {
        let parent = yaml("a");
        let child = yaml("b");
        assert_eq!(merge(&parent, &child), yaml("b"));
    }

    #[test]
    fn maps_recursive_merge() {
        let parent = yaml("{x: 1, y: 2}");
        let child = yaml("{y: 3, z: 4}");
        assert_eq!(merge(&parent, &child), yaml("{x: 1, y: 3, z: 4}"));
    }

    #[test]
    fn list_child_replaces_parent() {
        let parent = yaml("[1, 2, 3]");
        let child = yaml("[4, 5]");
        assert_eq!(merge(&parent, &child), yaml("[4, 5]"));
    }

    #[test]
    fn nested_map_merge() {
        let parent = yaml("{a: {x: 1, y: 2}}");
        let child = yaml("{a: {y: 3}}");
        assert_eq!(merge(&parent, &child), yaml("{a: {x: 1, y: 3}}"));
    }

    #[test]
    fn merge_all_chain() {
        let vals = vec![yaml("{a: 1}"), yaml("{a: 2, b: 3}"), yaml("{b: 4}")];
        assert_eq!(merge_all(&vals), yaml("{a: 2, b: 4}"));
    }
}