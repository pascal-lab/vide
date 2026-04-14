use ide::{
    code_lens::CodeLensConfig,
    document_highlight::DocumentHighlightConfig,
    formatting::FmtConfig,
    hover::HoverConfig,
    inlay_hint::InlayHintConfig,
    references::ReferencesConfig,
    rename::RenameConfig,
    semantic_tokens::{SemaTokenConfig, SemaTokenPortConfig},
    signature_help::SignatureHelpConfig,
};
use serde::{Deserialize, Serialize};
use utils::{json::get_field, paths::Utf8PathBuf};

use super::Config;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FilesWatcherDef {
    Client,
    Notify,
    Server,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ScopeVisibility {
    Public,
    Private,
}

impl From<ScopeVisibility> for ide::ScopeVisibility {
    fn from(val: ScopeVisibility) -> Self {
        match val {
            ScopeVisibility::Public => ide::ScopeVisibility::Public,
            ScopeVisibility::Private => ide::ScopeVisibility::Private,
        }
    }
}

macro_rules! default_value {
    ($default:expr, $ty:ty) => {{
        let default_: $ty = $default;
        serde_json::to_string_pretty(&default_).unwrap()
    }};
}

macro_rules! config_data {
    ($sv:vis struct $name:ident {
         $($(#[doc=$_:literal])*
         $field:ident : $ty:ty = $default:expr,)*
    }) => {
        #[allow(non_snake_case)]
        #[derive(Debug, Clone, PartialEq, Eq)]
        $sv struct $name {
            $($sv $field: $ty,)*
        }

        impl $name {
            $sv fn from_json(mut json: serde_json::Value, error_sink: &mut Vec<(String, serde_json::Error)>) -> $name {
                $name {
                    $( $field: get_field(&mut json, error_sink, stringify!($field), default_value!($default, $ty)), )*
                }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self {
                    $($field: $default,)*
                }
            }
        }
    };
}

config_data! {
    pub(crate) struct UserConfig {
        /// These directories will be ignored. They are relative to the workspace
        /// root, and globs are not supported. You may also need to add the
        /// folders to Code's `files.watcherExclude`.
        files_excludeDirs: Vec<Utf8PathBuf> = vec![],
        /// Controls file watching.
        files_watcher: FilesWatcherDef = FilesWatcherDef::Client,
        /// Automatically refresh project info on toml changes
        workspace_auto_reload: bool = true,

        /// If true, symbols within a scope (except for ports) are private to other scopes.
        scope_visibility: ScopeVisibility = ScopeVisibility::Private,

        formatter_path: Option<Utf8PathBuf> = None,
        formatting_on_enter: bool = true,
        formatting_in_comments: bool = true,
        formatting_indent_width: usize = 4,
        formatter_args: Vec<String> = vec![
            "--indentation_spaces=4",
            "--failsafe_success=false",
        ].into_iter().map(String::from).collect(),

        inlayHints_port_connection_enable: bool = true,
        inlayHints_parameter_assignment_enable: bool = true,
        inlayHints_end_structure_enable: bool = true,

        lens_instantiations_enable: bool = true,

        semantic_tokens_port_clk_rst_enable: bool = true,
        semantic_tokens_port_input_output_enable: bool = true,

        semantic_diagnostics_enable: bool = true,

        signature_help_params_only: bool = false,
    }
}

impl Config {
    pub(crate) fn references(&self) -> ReferencesConfig {
        let scope_visibility = self.user_config.scope_visibility.into();
        ReferencesConfig::new(scope_visibility, None)
    }

    pub(crate) fn document_highlight(&self) -> DocumentHighlightConfig {
        let scope_visibility = self.user_config.scope_visibility.into();
        DocumentHighlightConfig { scope_visibility }
    }

    pub(crate) fn rename(&self) -> RenameConfig {
        let scope_visibility = self.user_config.scope_visibility.into();
        RenameConfig { scope_visibility }
    }

    pub(crate) fn fmt(&self) -> FmtConfig {
        let mut args = self.user_config.formatter_args.clone();
        args.push(format!("--indentation_spaces={}", self.user_config.formatting_indent_width));
        FmtConfig {
            executable: self.user_config.formatter_path.clone(),
            args,
            on_enter: self.user_config.formatting_on_enter,
            in_comments: self.user_config.formatting_in_comments,
        }
    }

    pub(crate) fn hover(&self) -> HoverConfig {
        HoverConfig { format: self.cli_hover_markdown_support() }
    }

    pub(crate) fn inlay_hint(&self) -> InlayHintConfig {
        InlayHintConfig {
            port_connection: self.user_config.inlayHints_port_connection_enable,
            parameter_assignment: self.user_config.inlayHints_parameter_assignment_enable,
            end_structure: self.user_config.inlayHints_end_structure_enable,
        }
    }

    pub(crate) fn code_lens(&self) -> CodeLensConfig {
        CodeLensConfig { instantiations: self.user_config.lens_instantiations_enable }
    }

    pub(crate) fn semantic_tokens(&self) -> SemaTokenConfig {
        SemaTokenConfig {
            port: SemaTokenPortConfig {
                clk_rst: self.user_config.semantic_tokens_port_clk_rst_enable,
                io: self.user_config.semantic_tokens_port_input_output_enable,
            },
        }
    }

    pub(crate) fn semantic_diagnostics_enabled(&self) -> bool {
        self.user_config.semantic_diagnostics_enable
    }

    pub(crate) fn signature_help(&self) -> SignatureHelpConfig {
        SignatureHelpConfig { params_only: self.user_config.signature_help_params_only }
    }
}

#[test]
fn check_default() {
    let json = serde_json::Value::Null;
    let mut errors = vec![];
    let user_cfg = UserConfig::from_json(json, &mut errors);
    assert_eq!(user_cfg, UserConfig::default());
}
