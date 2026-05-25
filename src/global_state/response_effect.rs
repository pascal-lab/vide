use lsp_types::{SemanticTokens, Url};
use parking_lot::Mutex;
use triomphe::Arc;

use super::GlobalState;

#[derive(Debug, Clone, Default)]
pub(crate) struct AcceptedResponseEffects {
    effects: Arc<Mutex<Vec<AcceptedResponseEffect>>>,
}

impl AcceptedResponseEffects {
    pub(crate) fn push(&self, effect: AcceptedResponseEffect) {
        self.effects.lock().push(effect);
    }

    pub(crate) fn take(&self) -> Vec<AcceptedResponseEffect> {
        std::mem::take(&mut *self.effects.lock())
    }
}

#[derive(Debug)]
pub(crate) enum AcceptedResponseEffect {
    CommitSemanticTokens { uri: Url, tokens: SemanticTokens },
}

impl AcceptedResponseEffect {
    pub(crate) fn apply(self, state: &mut GlobalState) {
        match self {
            AcceptedResponseEffect::CommitSemanticTokens { uri, tokens } => {
                state.semantic_tokens_cache.lock().insert(uri, tokens);
            }
        }
    }
}
