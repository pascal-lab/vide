mod slang_token_kind {
    include!(concat!(env!("OUT_DIR"), "/token_kind.rs"));
}
mod slang_trivia_kind {
    include!(concat!(env!("OUT_DIR"), "/trivia_kind.rs"));
}
pub use slang_token_kind::*;
pub use slang_trivia_kind::*;
