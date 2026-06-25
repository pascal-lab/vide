use super::*;

#[test]
fn unconfigured_workspace_goto_definition_uses_indexed_unopened_files() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let temp_dir = TempDir::new("unconfigured-index-goto");
    let child_path = temp_dir.path().join("child.sv");
    let top_path = temp_dir.path().join("top.sv");
    let top_text = "module top;\n  child u();\nendmodule\n";
    fs::write(&child_path, "module child;\nendmodule\n").unwrap();
    fs::write(&top_path, top_text).unwrap();

    let root_path = temp_dir.path().to_path_buf();
    let (client, server_thread) = spawn_test_workspace(root_path, pull_caps, UserConfig::default());
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    let child_uri = to_proto::url_from_abs_path(child_path.as_path()).unwrap();
    open_test_document(&client, top_uri.clone(), top_text);
    let _ = request_document_diagnostics(&client, top_uri.clone(), 1);

    let definition_uris = request_goto_definition_uris(&client, top_uri, top_text, "child u", 2);
    assert!(
        definition_uris.contains(&child_uri),
        "definition should include unopened child.sv from default index: {definition_uris:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn type_definition_request_uses_module_definition_navigation() {
    let temp_dir = TempDir::new("type-definition-module-nav");
    let rtl_dir = temp_dir.path().join("rtl");
    fs::create_dir_all(&rtl_dir).unwrap();

    let top_text = "module top;\n  child u_child();\nendmodule\n";
    let child_text = "module child;\nendmodule\n";

    fs::write(
        temp_dir.path().join("vide.toml"),
        "top_modules = [\"top\"]\nsources = [\"rtl/*.v\"]\ninclude_dirs = [\"rtl\"]\n",
    )
    .unwrap();
    let top_path = rtl_dir.join("top.v");
    let child_path = rtl_dir.join("child.v");
    fs::write(&top_path, top_text).unwrap();
    fs::write(&child_path, child_text).unwrap();

    let (client, server_thread) = spawn_test_workspace(
        temp_dir.path().to_path_buf(),
        ClientCapabilities::default(),
        UserConfig::default(),
    );
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    let child_uri = to_proto::url_from_abs_path(child_path.as_path()).unwrap();
    open_test_document(&client, top_uri.clone(), top_text);
    open_test_document(&client, child_uri.clone(), child_text);
    let _ = request_document_diagnostics(&client, top_uri.clone(), 1);

    let definition_uris =
        request_type_definition_uris(&client, top_uri, top_text, "child u_child", 2);
    assert!(
        definition_uris.contains(&child_uri),
        "typeDefinition should reach child.v through the advertised capability: {definition_uris:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn call_hierarchy_reports_module_instance_edges() {
    let temp_dir = TempDir::new("call-hierarchy-module-edges");
    let rtl_dir = temp_dir.path().join("rtl");
    fs::create_dir_all(&rtl_dir).unwrap();

    let top_text = "module top;\n  child u_child();\nendmodule\n";
    let child_text = "module child;\n  leaf u_leaf();\nendmodule\n";
    let leaf_text = "module leaf;\nendmodule\n";

    fs::write(
        temp_dir.path().join("vide.toml"),
        "top_modules = [\"top\"]\nsources = [\"rtl/*.v\"]\ninclude_dirs = [\"rtl\"]\n",
    )
    .unwrap();
    let top_path = rtl_dir.join("top.v");
    let child_path = rtl_dir.join("child.v");
    let leaf_path = rtl_dir.join("leaf.v");
    fs::write(&top_path, top_text).unwrap();
    fs::write(&child_path, child_text).unwrap();
    fs::write(&leaf_path, leaf_text).unwrap();

    let (client, server_thread) = spawn_test_workspace(
        temp_dir.path().to_path_buf(),
        ClientCapabilities::default(),
        UserConfig::default(),
    );
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    let child_uri = to_proto::url_from_abs_path(child_path.as_path()).unwrap();
    let leaf_uri = to_proto::url_from_abs_path(leaf_path.as_path()).unwrap();
    open_test_document(&client, top_uri.clone(), top_text);
    open_test_document(&client, child_uri.clone(), child_text);
    open_test_document(&client, leaf_uri.clone(), leaf_text);
    let _ = request_document_diagnostics(&client, top_uri.clone(), 1);

    let prepared = prepare_call_hierarchy(&client, child_uri.clone(), child_text, "child;", 2);
    let child_item = prepared
        .into_iter()
        .find(|item| item.name == "child")
        .unwrap_or_else(|| panic!("child module should prepare call hierarchy item"));
    assert_eq!(child_item.kind, lsp_types::SymbolKind::MODULE);

    let incoming = request_call_hierarchy_incoming(&client, child_item.clone(), 3);
    assert!(
        incoming.iter().any(|call| {
            call.from.name == "top"
                && call.from.uri == top_uri
                && call.from_ranges.contains(&range_of(top_text, "child"))
        }),
        "incoming calls should include top instantiating child: {incoming:?}"
    );

    let outgoing = request_call_hierarchy_outgoing(&client, child_item, 4);
    assert!(
        outgoing.iter().any(|call| {
            call.to.name == "leaf"
                && call.to.uri == leaf_uri
                && call.from_ranges.contains(&range_of(child_text, "leaf"))
        }),
        "outgoing calls should include child instantiating leaf: {outgoing:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn include_expanded_parameter_decls_keep_module_navigation_available() {
    let temp_dir = TempDir::new("include-param-module-nav");
    let rtl_dir = temp_dir.path().join("rtl");
    fs::create_dir_all(&rtl_dir).unwrap();

    let top_text = "module top;\n  child #(.WIDTH(64)) u_child();\nendmodule\n";
    let child_text = "module child #(\n`include \"params.vh\"\n) ();\nendmodule\n";

    fs::write(
        temp_dir.path().join("vide.toml"),
        "top_modules = [\"top\"]\nsources = [\"rtl/*.v\"]\ninclude_dirs = [\"rtl\"]\n",
    )
    .unwrap();
    fs::write(rtl_dir.join("params.vh"), "parameter WIDTH = 32\n").unwrap();
    let top_path = rtl_dir.join("top.v");
    let child_path = rtl_dir.join("child.v");
    fs::write(&top_path, top_text).unwrap();
    fs::write(&child_path, child_text).unwrap();

    let root_path = temp_dir.path().to_path_buf();
    let (client, server_thread) =
        spawn_test_workspace(root_path, ClientCapabilities::default(), UserConfig::default());
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    let child_uri = to_proto::url_from_abs_path(child_path.as_path()).unwrap();

    open_test_document(&client, top_uri.clone(), top_text);
    open_test_document(&client, child_uri.clone(), child_text);
    let _ = request_document_diagnostics(&client, top_uri.clone(), 1);

    let definition_uris =
        request_goto_definition_uris(&client, top_uri.clone(), top_text, "child #", 2);
    assert!(
        definition_uris.contains(&child_uri),
        "go to definition should reach child.v despite include-expanded parameters: {definition_uris:?}"
    );

    let reference_uris =
        request_reference_uris(&client, child_uri.clone(), child_text, "child #", 3);
    assert!(
        reference_uris.contains(&child_uri) && reference_uris.contains(&top_uri),
        "references should include the module declaration and instantiation: {reference_uris:?}"
    );

    let lenses = request_code_lenses(&client, child_uri, 4);
    let lens = lenses.into_iter().next().expect("child module should have an instance code lens");
    let resolved = resolve_code_lens(&client, lens, 5);
    let title = resolved.command.expect("resolved code lens should have a command").title;
    assert_eq!(title, "1 instance");

    shutdown_test_server(&client, server_thread);
}

#[test]
fn include_defined_macro_powers_lsp_ide_features() {
    let temp_dir = TempDir::new("include-macro-lsp-features");
    let rtl_dir = temp_dir.path().join("rtl");
    let include_dir = temp_dir.path().join("include");
    fs::create_dir_all(&rtl_dir).unwrap();
    fs::create_dir_all(&include_dir).unwrap();

    let top_text = r#"`include "defs.vh"
`ifndef HEADER_FLAG
wire disabled_by_header;
`endif
module top;
  logic [`HEADER_WIDTH-1:0] data;
  localparam int W = `HEA;
endmodule
"#;
    let header_text = "`define HEADER_WIDTH 8\n`define HEADER_FLAG\n";

    fs::write(
        temp_dir.path().join("vide.toml"),
        "top_modules = [\"top\"]\nsources = [\"rtl/*.v\"]\ninclude_dirs = [\"include\"]\n",
    )
    .unwrap();
    let top_path = rtl_dir.join("top.v");
    let header_path = include_dir.join("defs.vh");
    fs::write(&top_path, top_text).unwrap();
    fs::write(&header_path, header_text).unwrap();

    let (client, server_thread) = spawn_test_workspace(
        temp_dir.path().to_path_buf(),
        ClientCapabilities::default(),
        UserConfig::default(),
    );
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    let header_uri = to_proto::url_from_abs_path(header_path.as_path()).unwrap();
    open_test_document(&client, top_uri.clone(), top_text);
    open_test_document(&client, header_uri.clone(), header_text);

    let (_result_id, diagnostics) = request_document_diagnostics(&client, top_uri.clone(), 1);
    assert!(
        diagnostics.iter().any(|diag| diag.message.contains("inactive")),
        "header define should drive inactive branch diagnostics: {diagnostics:?}"
    );

    let definition_uris =
        request_goto_definition_uris(&client, top_uri.clone(), top_text, "HEADER_WIDTH-1", 2);
    assert!(
        definition_uris.contains(&header_uri),
        "macro goto should reach included header definition: {definition_uris:?}"
    );

    let hover = request_hover(&client, top_uri.clone(), top_text, "HEADER_WIDTH-1", 3)
        .expect("macro hover expected");
    let hover_text = format!("{:?}", hover.contents);
    assert!(
        hover_text.contains("HEADER_WIDTH"),
        "macro hover should mention header macro name: {hover_text}"
    );

    let reference_uris =
        request_reference_uris(&client, top_uri.clone(), top_text, "HEADER_WIDTH-1", 4);
    assert!(
        reference_uris.contains(&top_uri) && reference_uris.contains(&header_uri),
        "macro references should include top use and header definition: {reference_uris:?}"
    );

    let completion_labels =
        request_completion_labels(&client, top_uri, top_text, ";\nendmodule", 5);
    assert!(
        completion_labels.iter().any(|label| label == "HEADER_WIDTH"),
        "completion should include macro from included header: {completion_labels:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn manifest_defined_macro_powers_lsp_ide_features() {
    let temp_dir = TempDir::new("manifest-macro-lsp-features");
    let rtl_dir = temp_dir.path().join("rtl");
    fs::create_dir_all(&rtl_dir).unwrap();

    let top_text = r#"`ifdef FROM_MANIFEST
module top;
  localparam int W = `FROM_MANIFEST;
endmodule
`endif
"#;
    let manifest_text =
        "top_modules = [\"top\"]\nsources = [\"rtl/*.sv\"]\ndefines = [\"FROM_MANIFEST=1\"]\n";

    let top_path = rtl_dir.join("top.sv");
    let manifest_path = temp_dir.path().join("vide.toml");
    fs::write(&top_path, top_text).unwrap();
    fs::write(&manifest_path, manifest_text).unwrap();

    let (client, server_thread) = spawn_test_workspace(
        temp_dir.path().to_path_buf(),
        ClientCapabilities::default(),
        UserConfig::default(),
    );
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    let manifest_uri = to_proto::url_from_abs_path(manifest_path.as_path()).unwrap();
    open_test_document(&client, top_uri.clone(), top_text);

    let (_result_id, diagnostics) = request_document_diagnostics(&client, top_uri.clone(), 1);
    assert!(
        diagnostics.iter().all(|diag| !diag.message.contains("unknown macro")),
        "manifest define should feed preprocessor diagnostics: {diagnostics:?}"
    );

    let definition_uris =
        request_goto_definition_uris(&client, top_uri.clone(), top_text, "FROM_MANIFEST;", 2);
    assert!(
        definition_uris.contains(&manifest_uri),
        "manifest macro goto should reach vide.toml define: {definition_uris:?}"
    );

    let hover = request_hover(&client, top_uri.clone(), top_text, "FROM_MANIFEST;", 3)
        .expect("manifest macro hover expected from source use");
    let hover_text = format!("{:?}", hover.contents);
    assert!(
        hover_text.contains("FROM_MANIFEST"),
        "manifest macro hover should mention macro name: {hover_text}"
    );

    let manifest_hover =
        request_hover(&client, manifest_uri.clone(), manifest_text, "FROM_MANIFEST=1", 4)
            .expect("manifest macro hover expected from manifest define");
    let manifest_hover_text = format!("{:?}", manifest_hover.contents);
    assert!(
        manifest_hover_text.contains("FROM_MANIFEST"),
        "manifest define hover should mention macro name: {manifest_hover_text}"
    );

    let manifest_definition_uris = request_goto_definition_uris(
        &client,
        manifest_uri.clone(),
        manifest_text,
        "FROM_MANIFEST=1",
        5,
    );
    assert!(
        manifest_definition_uris.contains(&manifest_uri),
        "manifest define should be linkable to itself: {manifest_definition_uris:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn references_request_respects_include_declaration() {
    let temp_dir = TempDir::new("references-include-declaration");
    let rtl_dir = temp_dir.path().join("rtl");
    fs::create_dir_all(&rtl_dir).unwrap();

    let top_text = "module top;\n  child u_child();\nendmodule\n";
    let child_text = "module child();\nendmodule\n";

    fs::write(
        temp_dir.path().join("vide.toml"),
        "top_modules = [\"top\"]\nsources = [\"rtl/*.v\"]\ninclude_dirs = [\"rtl\"]\n",
    )
    .unwrap();
    let top_path = rtl_dir.join("top.v");
    let child_path = rtl_dir.join("child.v");
    fs::write(&top_path, top_text).unwrap();
    fs::write(&child_path, child_text).unwrap();

    let root_path = temp_dir.path().to_path_buf();
    let (client, server_thread) =
        spawn_test_workspace(root_path, ClientCapabilities::default(), UserConfig::default());
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    let child_uri = to_proto::url_from_abs_path(child_path.as_path()).unwrap();

    open_test_document(&client, top_uri.clone(), top_text);
    open_test_document(&client, child_uri.clone(), child_text);
    let _ = request_document_diagnostics(&client, top_uri.clone(), 1);

    let refs_with_decl = request_reference_uris_with_include_declaration(
        &client,
        child_uri.clone(),
        child_text,
        "child()",
        2,
        true,
    );
    assert!(
        refs_with_decl.contains(&child_uri) && refs_with_decl.contains(&top_uri),
        "include_declaration=true should include the declaration and instantiation: {refs_with_decl:?}"
    );

    let refs_without_decl = request_reference_uris_with_include_declaration(
        &client,
        child_uri.clone(),
        child_text,
        "child()",
        3,
        false,
    );
    assert!(
        !refs_without_decl.contains(&child_uri) && refs_without_decl.contains(&top_uri),
        "include_declaration=false should exclude the declaration while keeping instantiations: {refs_without_decl:?}"
    );

    shutdown_test_server(&client, server_thread);
}
