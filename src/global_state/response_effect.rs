use lsp_types::{SemanticTokens, Url};

use super::GlobalState;

pub(crate) type AcceptedResponseEffects = vide_lsp_runtime::AcceptedEffects<AcceptedResponseEffect>;

#[derive(Debug)]
pub(crate) enum AcceptedResponseEffect {
    CommitSemanticTokens { uri: Url, tokens: SemanticTokens },
}

impl AcceptedResponseEffect {
    pub(crate) fn apply(self, state: &mut GlobalState) {
        match self {
            AcceptedResponseEffect::CommitSemanticTokens { uri, tokens } => {
                state.analysis.semantic_tokens_cache.lock().insert(uri, tokens);
            }
        }
    }
}
