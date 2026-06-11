use super::*;

#[test]
fn vide_diagnostics_are_localized_for_chinese_locale() {
    let text = "\
module child;
endmodule

module child;
endmodule

module top;
  child ambiguous_child();
endmodule
";
    let temp_dir = TempDir::new("vide-i18n-diagnostic");
    let file_path = temp_dir.path().join("ambiguous.sv");
    fs::write(&file_path, text).unwrap();
    let root_path = temp_dir.path().to_path_buf();
    let config = test_server_config_with_i18n(
        root_path,
        ClientCapabilities::default(),
        UserConfig::default(),
        I18n::new(Locale::ZhCn),
    );
    let (server, client) = Connection::memory();
    let server_thread = spawn_default_test_server(config, server);
    let uri = to_proto::url_from_abs_path(file_path.as_path()).unwrap();
    open_test_document(&client, uri.clone(), text);

    let (_result_id, diagnostics) = request_document_diagnostics(&client, uri, 220);
    assert!(
        diagnostics.iter().any(|diag| {
            diag.source.as_deref() == Some("vide")
                && diag.message.contains("模块实例化")
                && diag.message.contains("无法确定应使用哪一个")
                && !diag.message.contains("最佳努力索引")
                && !diag.message.contains("存在歧义")
        }),
        "expected localized Vide diagnostic message, got {diagnostics:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn code_actions_are_localized_for_chinese_locale() {
    let text = "\
module top;
  logic a, b;
endmodule
";
    let temp_dir = TempDir::new("vide-i18n-code-action");
    let file_path = temp_dir.path().join("split.sv");
    fs::write(&file_path, text).unwrap();
    let root_path = temp_dir.path().to_path_buf();
    let config = test_server_config_with_i18n(
        root_path,
        code_action_client_caps(),
        UserConfig::default(),
        I18n::new(Locale::ZhCn),
    );
    let (server, client) = Connection::memory();
    let server_thread = spawn_default_test_server(config, server);
    let uri = to_proto::url_from_abs_path(file_path.as_path()).unwrap();
    open_test_document(&client, uri.clone(), text);

    let actions = request_code_actions(
        &client,
        uri,
        text,
        "a, b",
        CodeActionContext {
            diagnostics: Vec::new(),
            only: Some(vec![CodeActionKind::REFACTOR_REWRITE]),
            trigger_kind: None,
        },
        221,
    );
    let titles = code_action_titles(&actions);

    assert!(
        titles.iter().any(|title| title == "拆分声明"),
        "expected localized code action title, got {titles:?}"
    );

    shutdown_test_server(&client, server_thread);
}
