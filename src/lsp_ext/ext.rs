use std::ops;

use lsp_types::notification::Notification;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeLensData {
    pub version: i32,
    pub kind: CodeLensDataKind,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodeLensDataKind {
    Instantiation(lsp_types::TextDocumentPositionParams),
}

macro_rules! define_semantic_token_kind {
    (   ($name:ident: [$ty:ty] @ $mod:ident) =>
        standard {
            $($standard:ident),*$(,)?
        }
        custom {
            $(($custom:ident, $string:literal) $(=> $fallback:ident)?),*$(,)?
        }
    ) => {
        pub(crate) mod $mod {
            $(pub(crate) const $standard: $ty = <$ty>::$standard;)*
            $(pub(crate) const $custom: $ty = <$ty>::new($string);)*

            pub(crate) fn fallback(token: $ty) -> Option<$ty> {
                $(
                    if token == $custom {
                        None $(.or(Some($fallback)))?
                    } else
                )*
                { Some(token)}
            }
        }

        pub(crate) const $name: &[$ty] = &[
            $(self::$mod::$standard,)*
            $(self::$mod::$custom),*
        ];
    };
}

define_semantic_token_kind! {
    (SEMA_TOKENS_TYPES: [lsp_types::SemanticTokenType] @ sema_token_types) =>
    standard {
        COMMENT,
        DECORATOR,
        ENUM_MEMBER,
        ENUM,
        FUNCTION,
        INTERFACE,
        KEYWORD,
        MACRO,
        METHOD,
        NAMESPACE,
        NUMBER,
        OPERATOR,
        PARAMETER,
        PROPERTY,
        STRING,
        STRUCT,
        TYPE_PARAMETER,
        VARIABLE,
        TYPE,
    }

    custom {
        (CLK_PORT, "port_clock") => KEYWORD,
        (RST_PORT, "port_reset") => KEYWORD,
        (OTHERS_PORT, "port_generic") => PARAMETER,
        (INSTANCE, "instance") => VARIABLE,
        (TYPE_ALIAS, "type_alias") => TYPE,
        (GENERIC, "generic") => TYPE_PARAMETER,
    }
}
#[derive(Default)]
pub(crate) struct SemaTokenModifierSet(pub(crate) u32);

impl SemaTokenModifierSet {
    pub(crate) fn finish(self) -> u32 {
        self.0
    }
}

define_semantic_token_kind! {
    (SEMA_TOKENS_MODIFIERS: [lsp_types::SemanticTokenModifier] @ sema_token_modifiers) =>
    standard {
        DECLARATION,
        DEFINITION,
        READONLY,
        STATIC,
        DEPRECATED,
        ABSTRACT,
        ASYNC,
        MODIFICATION,
        DOCUMENTATION,
        DEFAULT_LIBRARY,
    }
    custom {
        (READ, "read") => READONLY,
        (WRITE, "write") => MODIFICATION,
        (REF, "ref") => MODIFICATION,
        (DEF, "definition"),
    }
}

impl ops::BitOrAssign<lsp_types::SemanticTokenModifier> for SemaTokenModifierSet {
    fn bitor_assign(&mut self, rhs: lsp_types::SemanticTokenModifier) {
        let Some(idx) = SEMA_TOKENS_MODIFIERS.iter().position(|it| it == &rhs) else {
            tracing::debug!(?rhs, "unknown semantic token modifier");
            return;
        };
        self.0 |= 1 << idx;
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeActionData {
    pub code_action_params: lsp_types::CodeActionParams,
    pub id: String,
    pub version: Option<i32>,
}

#[derive(Debug, Error)]
pub enum CodeActionResolveError {
    #[error("code action without data")]
    NoData,
    #[error("stale code action")]
    Stable,
    #[error("invalid action id: {0}")]
    InvalidId(String),
}

pub const RUN_QIHE_ANALYSIS_COMMAND: &str = "vizsla.server.runQiheAnalysis";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunQiheAnalysisParams {
    pub uri: lsp_types::Url,
    #[serde(default)]
    pub cwd: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QiheStatusParams {
    pub token: String,
    pub state: String,
    pub message: Option<String>,
}

pub enum QiheStatusNotification {}

impl Notification for QiheStatusNotification {
    type Params = QiheStatusParams;

    const METHOD: &'static str = "vizsla/qiheStatus";
}
