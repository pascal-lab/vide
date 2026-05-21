use ide::rename::RenameError;

use crate::lsp_ext::ext::CodeActionResolveError;

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

    pub(crate) fn fetching_workspaces(self) -> &'static str {
        match self {
            Self::En => "Fetching Workspaces",
            Self::ZhCn => "正在获取工作区",
        }
    }

    pub(crate) fn roots_scanning(self) -> &'static str {
        match self {
            Self::En => "Roots Scanning",
            Self::ZhCn => "正在扫描根目录",
        }
    }

    pub(crate) fn qihe_progress_title(self) -> &'static str {
        match self {
            Self::En => "Running Qihe Analysis",
            Self::ZhCn => "正在运行 Qihe 分析",
        }
    }

    pub(crate) fn qihe_finished(self, total: usize) -> String {
        match self {
            Self::En => format!("Qihe analysis finished with {total} diagnostic(s)."),
            Self::ZhCn => format!("Qihe 分析完成，共 {total} 条诊断。"),
        }
    }

    pub(crate) fn qihe_failed(self) -> &'static str {
        match self {
            Self::En => "Qihe analysis failed",
            Self::ZhCn => "Qihe 分析失败",
        }
    }

    pub(crate) fn qihe_cancelled(self) -> &'static str {
        match self {
            Self::En => "qihe analysis cancelled",
            Self::ZhCn => "Qihe 分析已取消",
        }
    }

    pub(crate) fn qihe_location(self, primary_element: &str) -> String {
        match self {
            Self::En => format!("Location: {primary_element}"),
            Self::ZhCn => format!("位置：{primary_element}"),
        }
    }

    pub(crate) fn qihe_convert_diagnostic_failed(self) -> &'static str {
        match self {
            Self::En => "failed to convert qihe diagnostic",
            Self::ZhCn => "无法转换 Qihe 诊断",
        }
    }

    pub(crate) fn qihe_prepare_workspace_failed(self) -> &'static str {
        match self {
            Self::En => "failed to prepare qihe workspace",
            Self::ZhCn => "无法准备 Qihe 工作区",
        }
    }

    pub(crate) fn qihe_command_failed_to_start(self, label: &str, command_line: &str) -> String {
        match self {
            Self::En => format!("{label} failed to start: {command_line}"),
            Self::ZhCn => format!("{label} 启动失败：{command_line}"),
        }
    }

    pub(crate) fn qihe_command_failed(
        self,
        label: &str,
        status: std::process::ExitStatus,
        command_line: &str,
        stdout: &str,
        stderr: &str,
    ) -> String {
        match self {
            Self::En => format!(
                "{label} failed with status {status}.\ncommand:\n{command_line}\nstdout:\n{stdout}\nstderr:\n{stderr}"
            ),
            Self::ZhCn => format!(
                "{label} 失败，退出状态为 {status}。\n命令：\n{command_line}\n标准输出：\n{stdout}\n标准错误：\n{stderr}"
            ),
        }
    }

    pub(crate) fn qihe_read_diagnostics_failed(self, path: &std::path::Path) -> String {
        match self {
            Self::En => format!("failed to read qihe diagnostics at {}", path.display()),
            Self::ZhCn => format!("无法读取 Qihe 诊断文件 {}", path.display()),
        }
    }

    pub(crate) fn qihe_parse_diagnostics_failed(self, path: &std::path::Path) -> String {
        match self {
            Self::En => format!("failed to parse qihe diagnostics at {}", path.display()),
            Self::ZhCn => format!("无法解析 Qihe 诊断文件 {}", path.display()),
        }
    }

    pub(crate) fn qihe_read_diagnostics_dir_failed(self, path: &std::path::Path) -> String {
        match self {
            Self::En => format!("failed to read qihe diagnostics dir {}", path.display()),
            Self::ZhCn => format!("无法读取 Qihe 诊断目录 {}", path.display()),
        }
    }

    pub(crate) fn shutdown_already_requested(self) -> &'static str {
        match self {
            Self::En => "Shutdown already requested.",
            Self::ZhCn => "已请求关闭。",
        }
    }

    pub(crate) fn unknown_request(self) -> &'static str {
        match self {
            Self::En => "unknown request",
            Self::ZhCn => "未知请求",
        }
    }

    pub(crate) fn missing_execute_command_arguments(self) -> &'static str {
        match self {
            Self::En => "missing executeCommand arguments",
            Self::ZhCn => "缺少 executeCommand 参数",
        }
    }

    pub(crate) fn unknown_execute_command(self, command: &str) -> String {
        match self {
            Self::En => format!("unknown executeCommand: {command}"),
            Self::ZhCn => format!("未知 executeCommand：{command}"),
        }
    }

    pub(crate) fn instance_count(self, count: usize) -> String {
        match self {
            Self::En => {
                let s = if count == 1 { "" } else { "s" };
                format!("{count} instance{s}")
            }
            Self::ZhCn => format!("{count} 个实例"),
        }
    }

    pub(crate) fn code_action_title(self, id: &str, label: &str) -> String {
        if self == Self::En {
            return label.to_owned();
        }

        match id {
            "add_missing_connections" => "补全连接".to_owned(),
            "add_missing_parameters" => "补全参数".to_owned(),
            "convert_ordered_ports" => "将有序端口连接转换为命名连接".to_owned(),
            "convert_ordered_params" => "将有序参数赋值转换为命名赋值".to_owned(),
            "remove_empty_port_connections" => "移除空端口连接".to_owned(),
            "add_implicit_named_port_parens" => "添加显式空端口连接".to_owned(),
            "add_instance_parens" => "添加空实例端口列表".to_owned(),
            "convert_literal_base" => match label {
                "Convert literal to binary" => "将字面量转换为二进制",
                "Convert literal to octal" => "将字面量转换为八进制",
                "Convert literal to decimal" => "将字面量转换为十进制",
                "Convert literal to hexadecimal" => "将字面量转换为十六进制",
                _ => label,
            }
            .to_owned(),
            _ => label.to_owned(),
        }
    }

    pub(crate) fn rename_error(self, err: RenameError) -> &'static str {
        match (self, err) {
            (Self::En, RenameError::NoRefFound) => "No references found at position",
            (Self::En, RenameError::NoDefFound) => "No definitions found for the token",
            (Self::En, RenameError::OverlappingEdits) => "Generated overlapping edits",
            (Self::ZhCn, RenameError::NoRefFound) => "当前位置未找到引用",
            (Self::ZhCn, RenameError::NoDefFound) => "未找到该标记的定义",
            (Self::ZhCn, RenameError::OverlappingEdits) => "生成了相互重叠的编辑",
        }
    }

    pub(crate) fn code_action_resolve_error(self, err: CodeActionResolveError) -> String {
        match (self, err) {
            (Self::En, CodeActionResolveError::NoData) => "code action without data".to_owned(),
            (Self::En, CodeActionResolveError::Stable) => "stale code action".to_owned(),
            (Self::En, CodeActionResolveError::InvalidId(id)) => {
                format!("invalid action id: {id}")
            }
            (Self::ZhCn, CodeActionResolveError::NoData) => "代码操作缺少数据".to_owned(),
            (Self::ZhCn, CodeActionResolveError::Stable) => "代码操作已过期".to_owned(),
            (Self::ZhCn, CodeActionResolveError::InvalidId(id)) => {
                format!("无效的操作 ID：{id}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Locale;

    #[test]
    fn maps_lsp_locales_to_supported_locales() {
        assert_eq!(Locale::from_lsp(Some("zh-CN")), Locale::ZhCn);
        assert_eq!(Locale::from_lsp(Some("zh_Hans")), Locale::ZhCn);
        assert_eq!(Locale::from_lsp(Some("en-US")), Locale::En);
        assert_eq!(Locale::from_lsp(None), Locale::En);
    }

    #[test]
    fn localizes_lsp_code_action_titles() {
        assert_eq!(
            Locale::ZhCn.code_action_title(
                "convert_ordered_ports",
                "Convert ordered port connections to named connections",
            ),
            "将有序端口连接转换为命名连接"
        );
        assert_eq!(
            Locale::ZhCn
                .code_action_title("convert_literal_base", "Convert literal to hexadecimal",),
            "将字面量转换为十六进制"
        );
        assert_eq!(
            Locale::En.code_action_title("add_missing_connections", "Fill connections"),
            "Fill connections"
        );
    }
}
