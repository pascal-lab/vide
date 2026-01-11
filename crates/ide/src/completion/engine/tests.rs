use base_db::{change::Change, source_root::SourceRoot};
use triomphe::Arc;
use utils::{lines::LineEnding, text_edit::TextSize};
use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

use super::*;
use crate::analysis_host::AnalysisHost;

fn setup(text: &str) -> (AnalysisHost, FilePosition) {
    let marker = "/*caret*/";
    let off = text.find(marker).expect("missing /*caret*/");
    let mut owned = text.to_string();
    owned = owned.replace(marker, "");

    let file_id = FileId(0);
    let path = VfsPath::new_virtual_path("/test.v".to_string());

    let mut file_set = FileSet::default();
    file_set.insert(file_id, path);
    let root = SourceRoot::new_local(file_set);

    let mut change = Change::new();
    change.set_roots(vec![root]);
    change.add_changed_file(ChangedFile {
        file_id,
        change_kind: ChangeKind::Create(Arc::from(owned.as_str()), LineEnding::Unix),
    });

    let mut host = AnalysisHost::default();
    host.apply_change(change);
    let position = FilePosition { file_id, offset: TextSize::from(off as u32) };
    (host, position)
}

fn completions_in_text(text: &str) -> Vec<CompletionItem> {
    let (host, position) = setup(text);
    super::completions(host.raw_db(), position, None)
}

#[test]
fn filters_named_port_connection_expr_by_width() {
    let items = completions_in_text(
        "module m(input [3:0] a); endmodule\n\
         module top;\n\
         wire [3:0] sig4;\n\
         wire [7:0] sig8;\n\
         wire sig1;\n\
         m u0(.a(/*caret*/));\n\
         endmodule\n",
    );
    let labels: Vec<_> = items.into_iter().map(|it| it.label).collect();
    assert!(labels.contains(&"sig4".to_string()));
    assert!(!labels.contains(&"sig8".to_string()));
    assert!(!labels.contains(&"sig1".to_string()));
}

#[test]
fn filters_named_param_assign_expr_by_width() {
    let items = completions_in_text(
        "module m #(parameter [3:0] W = 4) (); endmodule\n\
         module top;\n\
         localparam [3:0] P4 = 4;\n\
         localparam [7:0] P8 = 8;\n\
         m #(.W(/*caret*/)) u0();\n\
         endmodule\n",
    );
    let labels: Vec<_> = items.into_iter().map(|it| it.label).collect();
    assert!(labels.contains(&"P4".to_string()));
    assert!(!labels.contains(&"P8".to_string()));
}
