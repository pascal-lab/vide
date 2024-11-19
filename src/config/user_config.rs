use ide::{
    document_highlight::DocumentHighlightConfig, formatting::FmtConfig,
    references::ReferencesConfig, rename::RenameConfig,
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
        &serde_json::to_string_pretty(&default_).unwrap()
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
    }
}

impl Config {
    pub(crate) fn references_config(&self) -> ReferencesConfig {
        let scope_visibility = self.user_config.scope_visibility.into();
        ReferencesConfig::new(scope_visibility, None)
    }

    pub(crate) fn document_highlight_config(&self) -> DocumentHighlightConfig {
        let scope_visibility = self.user_config.scope_visibility.into();
        DocumentHighlightConfig { scope_visibility }
    }

    pub(crate) fn rename_config(&self) -> RenameConfig {
        let scope_visibility = self.user_config.scope_visibility.into();
        RenameConfig { scope_visibility }
    }

    pub(crate) fn fmt_config(&self) -> FmtConfig {
        let mut args = self.user_config.formatter_args.clone();
        args.push(format!("--indentation_spaces={}", self.user_config.formatting_indent_width));
        FmtConfig {
            executable: self.user_config.formatter_path.clone(),
            args,
            on_enter: self.user_config.formatting_on_enter,
            in_comments: self.user_config.formatting_in_comments,
        }
    }
}

#[test]
fn check_default() {
    let json = serde_json::Value::Null;
    let mut errors = vec![];
    let user_cfg = UserConfig::from_json(json, &mut errors);
    assert_eq!(user_cfg, UserConfig::default());
}
