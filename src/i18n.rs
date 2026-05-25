use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Locale {
    #[default]
    En,
    ZhCn,
}

impl Locale {
    pub(crate) fn from_lsp(locale: Option<&str>) -> Self {
        let Some(locale) = locale else {
            return Self::En;
        };

        let locale = locale.trim().to_ascii_lowercase().replace('_', "-");
        if locale == "zh" || locale.starts_with("zh-") { Self::ZhCn } else { Self::En }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct I18n {
    locale: Locale,
}

impl I18n {
    pub(crate) fn new(locale: Locale) -> Self {
        Self { locale }
    }

    pub(crate) fn text(self, key: &'static str) -> &'static str {
        lookup(self.locale, key).or_else(|| lookup(Locale::En, key)).unwrap_or(key)
    }

    pub(crate) fn format<'a>(
        self,
        key: &'static str,
        args: impl IntoIterator<Item = (&'a str, String)>,
    ) -> String {
        let mut message = self.text(key).to_owned();
        for (name, value) in args {
            message = message.replace(&format!("{{{name}}}"), &value);
        }
        message
    }
}

pub(crate) mod keys {
    pub(crate) const PROGRESS_FETCHING_WORKSPACES: &str = "progress.fetching_workspaces";
    pub(crate) const PROGRESS_ROOTS_SCANNING: &str = "progress.roots_scanning";

    pub(crate) const QIHE_PROGRESS_TITLE: &str = "qihe.progress_title";
    pub(crate) const QIHE_FINISHED: &str = "qihe.finished";
    pub(crate) const QIHE_FAILED: &str = "qihe.failed";
    pub(crate) const QIHE_CANCELLED: &str = "qihe.cancelled";
    pub(crate) const QIHE_STALE: &str = "qihe.stale";
    pub(crate) const QIHE_LOCATION: &str = "qihe.location";
    pub(crate) const QIHE_CONVERT_DIAGNOSTIC_FAILED: &str = "qihe.convert_diagnostic_failed";
    pub(crate) const QIHE_PREPARE_WORKSPACE_FAILED: &str = "qihe.prepare_workspace_failed";
    pub(crate) const QIHE_COMMAND_FAILED_TO_START: &str = "qihe.command_failed_to_start";
    pub(crate) const QIHE_COMMAND_FAILED: &str = "qihe.command_failed";
    pub(crate) const QIHE_READ_DIAGNOSTICS_FAILED: &str = "qihe.read_diagnostics_failed";
    pub(crate) const QIHE_PARSE_DIAGNOSTICS_FAILED: &str = "qihe.parse_diagnostics_failed";
    pub(crate) const QIHE_READ_DIAGNOSTICS_DIR_FAILED: &str = "qihe.read_diagnostics_dir_failed";

    pub(crate) const SERVER_SHUTDOWN_ALREADY_REQUESTED: &str = "server.shutdown_already_requested";
    pub(crate) const SERVER_UNKNOWN_REQUEST: &str = "server.unknown_request";

    pub(crate) const EXECUTE_COMMAND_MISSING_ARGUMENTS: &str = "execute_command.missing_arguments";
    pub(crate) const EXECUTE_COMMAND_UNKNOWN: &str = "execute_command.unknown";

    pub(crate) const CONFIG_INVALID_VALUE_ONE: &str = "config.invalid_value_one";
    pub(crate) const CONFIG_INVALID_VALUE_MANY: &str = "config.invalid_value_many";

    pub(crate) const CODE_LENS_INSTANCES_ONE: &str = "code_lens.instances_one";
    pub(crate) const CODE_LENS_INSTANCES_MANY: &str = "code_lens.instances_many";

    pub(crate) const CODE_ACTION_ADD_MISSING_CONNECTIONS: &str =
        "code_action.add_missing_connections";
    pub(crate) const CODE_ACTION_ADD_MISSING_PARAMETERS: &str =
        "code_action.add_missing_parameters";
    pub(crate) const CODE_ACTION_CONVERT_ORDERED_PORTS: &str = "code_action.convert_ordered_ports";
    pub(crate) const CODE_ACTION_CONVERT_ORDERED_PARAMS: &str =
        "code_action.convert_ordered_params";
    pub(crate) const CODE_ACTION_REMOVE_EMPTY_PORT_CONNECTIONS: &str =
        "code_action.remove_empty_port_connections";
    pub(crate) const CODE_ACTION_ADD_IMPLICIT_NAMED_PORT_PARENS: &str =
        "code_action.add_implicit_named_port_parens";
    pub(crate) const CODE_ACTION_ADD_INSTANCE_PARENS: &str = "code_action.add_instance_parens";
    pub(crate) const CODE_ACTION_CONVERT_LITERAL_TO_BINARY: &str =
        "code_action.convert_literal_to_binary";
    pub(crate) const CODE_ACTION_CONVERT_LITERAL_TO_OCTAL: &str =
        "code_action.convert_literal_to_octal";
    pub(crate) const CODE_ACTION_CONVERT_LITERAL_TO_DECIMAL: &str =
        "code_action.convert_literal_to_decimal";
    pub(crate) const CODE_ACTION_CONVERT_LITERAL_TO_HEXADECIMAL: &str =
        "code_action.convert_literal_to_hexadecimal";

    pub(crate) const RENAME_NO_REF_FOUND: &str = "rename.no_ref_found";
    pub(crate) const RENAME_NO_DEF_FOUND: &str = "rename.no_def_found";
    pub(crate) const RENAME_OVERLAPPING_EDITS: &str = "rename.overlapping_edits";

    pub(crate) const CODE_ACTION_RESOLVE_NO_DATA: &str = "code_action_resolve.no_data";
    pub(crate) const CODE_ACTION_RESOLVE_STALE: &str = "code_action_resolve.stale";
    pub(crate) const CODE_ACTION_RESOLVE_INVALID_ID: &str = "code_action_resolve.invalid_id";
}

static EN_MESSAGES: LazyLock<toml::Table> =
    LazyLock::new(|| load_messages(include_str!("i18n/en.toml")));
static ZH_CN_MESSAGES: LazyLock<toml::Table> =
    LazyLock::new(|| load_messages(include_str!("i18n/zh-CN.toml")));

fn load_messages(text: &str) -> toml::Table {
    toml::from_str(text).expect("embedded i18n table must be valid TOML")
}

fn lookup(locale: Locale, key: &str) -> Option<&'static str> {
    let table = match locale {
        Locale::En => &*EN_MESSAGES,
        Locale::ZhCn => &*ZH_CN_MESSAGES,
    };
    lookup_in_table(table, key)
}

fn lookup_in_table<'a>(table: &'a toml::Table, key: &str) -> Option<&'a str> {
    let mut parts = key.split('.');
    let mut value = table.get(parts.next()?)?;
    for part in parts {
        value = value.as_table()?.get(part)?;
    }
    value.as_str()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use toml::Value;

    use super::{I18n, Locale, keys};

    #[test]
    fn maps_lsp_locales_to_supported_locales() {
        assert_eq!(Locale::from_lsp(Some("zh-CN")), Locale::ZhCn);
        assert_eq!(Locale::from_lsp(Some("zh_Hans")), Locale::ZhCn);
        assert_eq!(Locale::from_lsp(Some("en-US")), Locale::En);
        assert_eq!(Locale::from_lsp(None), Locale::En);
    }

    #[test]
    fn reads_messages_from_embedded_tables() {
        assert_eq!(
            I18n::new(Locale::ZhCn).text(keys::CODE_ACTION_CONVERT_ORDERED_PORTS),
            "将有序端口连接转换为命名连接"
        );
        assert_eq!(
            I18n::new(Locale::En).text(keys::CODE_ACTION_ADD_MISSING_CONNECTIONS),
            "Fill connections"
        );
    }

    #[test]
    fn formats_named_args() {
        assert_eq!(
            I18n::new(Locale::ZhCn).format(keys::QIHE_FINISHED, [("total", 3.to_string())]),
            "Qihe 分析完成，共 3 条诊断。"
        );
    }

    #[test]
    fn locale_tables_have_matching_keys() {
        let en_keys = leaf_keys(&super::EN_MESSAGES);
        let zh_cn_keys = leaf_keys(&super::ZH_CN_MESSAGES);

        assert_eq!(en_keys, zh_cn_keys);
    }

    fn leaf_keys(table: &toml::Table) -> BTreeSet<String> {
        let mut keys = BTreeSet::new();
        collect_leaf_keys("", table, &mut keys);
        keys
    }

    fn collect_leaf_keys(prefix: &str, table: &toml::Table, keys: &mut BTreeSet<String>) {
        for (key, value) in table {
            let path = if prefix.is_empty() { key.clone() } else { format!("{prefix}.{key}") };

            match value {
                Value::String(_) => {
                    keys.insert(path);
                }
                Value::Table(table) => collect_leaf_keys(&path, table, keys),
                _ => panic!("i18n value at {path} must be a string or table"),
            }
        }
    }
}
