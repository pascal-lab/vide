use super::*;

#[test]
fn system_call_inlay_hints_annotate_arguments() {
    let text = "module m;\ninitial begin\n  $display(\"x=%d\", x);\n  $readmemh(\"mem.hex\", mem);\nend\nendmodule\n";
    let (_temp_dir, client, server_thread, uris) = setup_configured_multi_file_diagnostics_test(
        ClientCapabilities::default(),
        UserConfig::default(),
        &[("top.sv", text)],
    );
    let top_uri = uris[0].clone();
    let _ = request_document_diagnostics(&client, top_uri.clone(), 1);

    let request_id = lsp_server::RequestId::from(2);
    client
        .sender
        .send(Message::Request(Request::new(
            request_id.clone(),
            InlayHintRequest::METHOD.to_string(),
            InlayHintParams {
                text_document: TextDocumentIdentifier { uri: top_uri },
                range: Range::new(Position::new(0, 0), Position::new(5, 0)),
                work_done_progress_params: WorkDoneProgressParams::default(),
            },
        )))
        .unwrap();
    let hints: Option<Vec<lsp_types::InlayHint>> = recv_response(&client, request_id, "inlay_hint");
    let hints = hints.expect("inlay hints for system calls");
    let labels: Vec<String> = hints
        .iter()
        .map(|hint| match &hint.label {
            lsp_types::InlayHintLabel::String(label) => label.clone(),
            lsp_types::InlayHintLabel::LabelParts(parts) => {
                parts.iter().map(|part| part.value.clone()).collect()
            }
        })
        .collect();
    // `endmodule : m` is the end-structure hint; the system call hints are
    // the parameter labels.
    let sys_labels: Vec<String> =
        labels.iter().filter(|label| !label.starts_with(':')).cloned().collect();
    assert_eq!(
        sys_labels,
        vec!["format:".to_string(), "file:".to_string(), "mem:".to_string()],
        "fixed parameters must be annotated, variadic tail skipped: {labels:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn signature_help_reaches_forward_declared_functions() {
    // A call reference searches every scope to its end (26.3), so a function
    // declared after the call still provides signature help.
    let text = "module m;\nassign y = f(1);\nfunction int f(input int a);\nreturn a;\nendfunction\nendmodule\n";
    let (_temp_dir, client, server_thread, uris) = setup_configured_multi_file_diagnostics_test(
        ClientCapabilities::default(),
        UserConfig::default(),
        &[("top.sv", text)],
    );
    let top_uri = uris[0].clone();
    let _ = request_document_diagnostics(&client, top_uri.clone(), 1);

    let request_id = lsp_server::RequestId::from(2);
    client
        .sender
        .send(Message::Request(Request::new(
            request_id.clone(),
            SignatureHelpRequest::METHOD.to_string(),
            SignatureHelpParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: top_uri },
                    position: position_at_offset(text, text.find("f(1)").unwrap() + 2),
                },
                context: None,
                work_done_progress_params: WorkDoneProgressParams::default(),
            },
        )))
        .unwrap();
    let help: Option<lsp_types::SignatureHelp> =
        recv_response(&client, request_id, "signature_help");
    let help = help.expect("signature help for forward-declared function call");
    assert!(
        help.signatures.iter().any(|signature| signature.label.contains("f")),
        "forward-declared function must provide signature help: {help:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn goto_definition_prefers_activated_wildcard_import_over_later_declaration() {
    // IEEE 1800-2017 26.3 Example 1: the reference activates p::x, so
    // goto-definition must not jump to the later declaration of x.
    let pull_caps = ClientCapabilities::default();
    let temp_dir = TempDir::new("goto-wildcard-point");
    let top_path = temp_dir.path().join("top.sv");
    let top_text = "package p;\nint x;\nendpackage\nmodule top;\nimport p::*;\nif (1) begin : b\n  initial x = 1;\nend\nint x;\nendmodule\n";
    fs::write(&top_path, top_text).unwrap();

    let root_path = temp_dir.path().to_path_buf();
    let (client, server_thread) = spawn_test_workspace(root_path, pull_caps, UserConfig::default());
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    open_test_document(&client, top_uri.clone(), top_text);
    let _ = request_document_diagnostics(&client, top_uri.clone(), 1);

    let definition_uris =
        request_goto_definition_uris(&client, top_uri.clone(), top_text, "x = 1", 2);
    assert_eq!(definition_uris, vec![top_uri.clone()]);

    // A scalar/array goto-definition response carries the target range;
    // re-request and compare against the package declaration's range.
    let request_id = lsp_server::RequestId::from(3);
    client
        .sender
        .send(Message::Request(Request::new(
            request_id.clone(),
            GotoDefinition::METHOD.to_string(),
            GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: top_uri },
                    position: position_of(top_text, "x = 1"),
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();
    let definition: Option<GotoDefinitionResponse> =
        recv_response(&client, request_id, "definition");
    let definition = definition.expect("definition response");
    let range = match definition {
        GotoDefinitionResponse::Scalar(location) => location.range,
        GotoDefinitionResponse::Array(locations) => locations[0].range,
        GotoDefinitionResponse::Link(_) => panic!("unexpected link response"),
    };
    assert_eq!(
        range,
        range_of(top_text, "x"),
        "goto-definition must resolve the reference to p::x, not the later declaration"
    );

    shutdown_test_server(&client, server_thread);
}

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
fn manifest_top_module_navigates_to_systemverilog_definition() {
    let temp_dir = TempDir::new("manifest-navigation");
    let manifest_text = "top_modules = [\"top\"]\nsources = [\"*.sv\"]\n";
    let top_text = "module top;\nendmodule\n";
    let manifest_path = temp_dir.path().join("vide.toml");
    let top_path = temp_dir.path().join("top.sv");
    fs::write(&manifest_path, manifest_text).unwrap();
    fs::write(&top_path, top_text).unwrap();

    let (client, server_thread) = spawn_test_workspace(
        temp_dir.path().to_path_buf(),
        ClientCapabilities::default(),
        UserConfig::default(),
    );
    let manifest_uri = to_proto::url_from_abs_path(manifest_path.as_path()).unwrap();
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    open_test_document(&client, manifest_uri.clone(), manifest_text);
    open_test_document(&client, top_uri.clone(), top_text);
    let _ = request_document_diagnostics(&client, manifest_uri.clone(), 1);

    let definition_uris =
        request_goto_definition_uris(&client, manifest_uri.clone(), manifest_text, "\"top\"", 2);
    assert_eq!(definition_uris, vec![top_uri.clone()]);

    let definition_uris =
        request_goto_definition_uris(&client, manifest_uri, manifest_text, "top_modules", 2);
    assert_eq!(definition_uris, vec![top_uri]);

    shutdown_test_server(&client, server_thread);
}

#[test]
fn manifest_top_module_reports_systemverilog_references() {
    let temp_dir = TempDir::new("manifest-references");
    let manifest_text = "top_modules = [\"child\"]\nsources = [\"*.sv\"]\n";
    let child_text = "module child;\nendmodule\n";
    let top_text = "module top;\n  child u_child();\nendmodule\n";
    let manifest_path = temp_dir.path().join("vide.toml");
    let child_path = temp_dir.path().join("child.sv");
    let top_path = temp_dir.path().join("top.sv");
    fs::write(&manifest_path, manifest_text).unwrap();
    fs::write(&child_path, child_text).unwrap();
    fs::write(&top_path, top_text).unwrap();

    let (client, server_thread) = spawn_test_workspace(
        temp_dir.path().to_path_buf(),
        ClientCapabilities::default(),
        UserConfig::default(),
    );
    let manifest_uri = to_proto::url_from_abs_path(manifest_path.as_path()).unwrap();
    let child_uri = to_proto::url_from_abs_path(child_path.as_path()).unwrap();
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    open_test_document(&client, manifest_uri.clone(), manifest_text);
    open_test_document(&client, child_uri.clone(), child_text);
    open_test_document(&client, top_uri.clone(), top_text);
    let _ = request_document_diagnostics(&client, manifest_uri.clone(), 1);

    let reference_uris =
        request_reference_uris(&client, manifest_uri.clone(), manifest_text, "\"child\"", 2);
    assert!(reference_uris.contains(&manifest_uri));
    assert!(reference_uris.contains(&child_uri));
    assert!(reference_uris.contains(&top_uri));

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
fn include_file_macro_hits_resolve_across_models() {
    // Issue #327: macro tokens in an include file belong to the including
    // file's trace. The dedicated macro reference/definition/parameter paths
    // iterate every context model, so all caret positions in the include
    // file must resolve (this is the reachability proof for the reported
    // cross-trace emitted-token gap).
    let temp_dir = TempDir::new("include-macro-hit");
    let rtl_dir = temp_dir.path().join("rtl");
    fs::create_dir_all(&rtl_dir).unwrap();

    let top_text = "`define WIDTH 32\n`include \"defs.vh\"\nmodule top;\n  assign y = `DOUBLE(`WIDTH);\nendmodule\n";
    let other_text = "`define WIDTH 64\n`include \"defs.vh\"\nmodule other;\n  assign z = `DOUBLE(`WIDTH);\nendmodule\n";
    let defs_text = "`define LOCAL 1\n`define DOUBLE(x) ((x)+(x))\nmodule defs_mod;\n  assign x = `WIDTH + `LOCAL;\nendmodule\n";
    fs::write(
        temp_dir.path().join("vide.toml"),
        "top_modules = [\"top\"]\nsources = [\"rtl/*.sv\"]\ninclude_dirs = [\"rtl\"]\n",
    )
    .unwrap();
    let top_path = rtl_dir.join("top.sv");
    let other_path = rtl_dir.join("other.sv");
    let defs_path = rtl_dir.join("defs.vh");
    fs::write(&top_path, top_text).unwrap();
    fs::write(&other_path, other_text).unwrap();
    fs::write(&defs_path, defs_text).unwrap();

    let root_path = temp_dir.path().to_path_buf();
    let (client, server_thread) =
        spawn_test_workspace(root_path, ClientCapabilities::default(), UserConfig::default());
    let defs_uri = to_proto::url_from_abs_path(defs_path.as_path()).unwrap();

    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    open_test_document(&client, top_uri.clone(), top_text);
    open_test_document(&client, defs_uri.clone(), defs_text);
    let _ = request_document_diagnostics(&client, top_uri.clone(), 1);

    // Same-file macro call in the including file: must resolve.
    let same_file = request_hover(&client, top_uri.clone(), top_text, "`WIDTH)", 2);
    assert!(same_file.is_some(), "same-file macro call must hover");
    // Self-contained macro in the include file: must resolve within its own trace.
    let self_contained = request_hover(&client, defs_uri.clone(), defs_text, "`LOCAL", 3);
    assert!(self_contained.is_some(), "self-contained include macro must hover");
    // Cross-file macro call in the include file: the issue scenario.
    let cross_file = request_hover(&client, defs_uri.clone(), defs_text, "`WIDTH", 4);
    assert!(cross_file.is_some(), "cross-file macro call in the include file must hover");
    // Caret on a token inside a macro BODY defined in the include file.
    let body_token = request_hover(&client, defs_uri.clone(), defs_text, "DOUBLE(x)", 5);
    assert!(body_token.is_some(), "macro body token in the include file must hover");
    // Two top files include the same header: multiple context models.
    let other_uri = to_proto::url_from_abs_path(other_path.as_path()).unwrap();
    open_test_document(&client, other_uri.clone(), other_text);
    let multi_model = request_hover(&client, defs_uri.clone(), defs_text, "`WIDTH", 6);
    assert!(multi_model.is_some(), "multi-model macro reference in the include file must hover");

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

    let manifest_reference_uris =
        request_reference_uris(&client, manifest_uri.clone(), manifest_text, "FROM_MANIFEST=1", 6);
    assert!(
        manifest_reference_uris.contains(&manifest_uri)
            && manifest_reference_uris.contains(&top_uri),
        "manifest macro references should include the config and source use: {manifest_reference_uris:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn manifest_formatting_returns_a_document_edit() {
    let temp_dir = TempDir::new("manifest-formatting");
    let manifest_text = "# project\ntop_modules=[\"top\"] # selected top\n";
    let manifest_path = temp_dir.path().join("vide.toml");
    fs::write(&manifest_path, manifest_text).unwrap();

    let (client, server_thread) = spawn_test_workspace(
        temp_dir.path().to_path_buf(),
        ClientCapabilities::default(),
        UserConfig::default(),
    );
    let manifest_uri = to_proto::url_from_abs_path(manifest_path.as_path()).unwrap();
    open_test_document(&client, manifest_uri.clone(), manifest_text);

    let request_id = lsp_server::RequestId::from(1);
    client
        .sender
        .send(Message::Request(Request::new(
            request_id.clone(),
            Formatting::METHOD.to_string(),
            DocumentFormattingParams {
                text_document: TextDocumentIdentifier { uri: manifest_uri },
                options: lsp_types::FormattingOptions::default(),
                work_done_progress_params: WorkDoneProgressParams::default(),
            },
        )))
        .unwrap();
    let edits: Option<Vec<lsp_types::TextEdit>> = recv_response(&client, request_id, "formatting");
    let edits = edits.expect("manifest formatting should return edits");
    assert_eq!(edits.len(), 1, "manifest formatting should use one full-document edit");
    assert_eq!(edits[0].new_text, "# project\ntop_modules = [\"top\"] # selected top\n");

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
