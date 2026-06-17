mod events;
mod helpers;
mod tokens;

use std::collections::BTreeMap;

use smol_str::{SmolStr, ToSmolStr};
use syntax::{
    SourceBufferOrigin, SourceBufferRange, SyntaxKind,
    preproc::{
        ActualArgument, EmittedToken, Event, EventId, MacroParam, Token, TokenOrigin, Trace,
    },
};
use utils::line_index::{TextRange, TextSize};

use super::*;

impl From<EventId> for SourcePreprocEventId {
    fn from(value: EventId) -> Self {
        Self(value.0)
    }
}
