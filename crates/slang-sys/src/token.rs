mod token_kind {
    include!(concat!(env!("OUT_DIR"), "/token_kind.rs"));
}
mod trivia_kind {
    include!(concat!(env!("OUT_DIR"), "/trivia_kind.rs"));
}
pub use token_kind::*;
pub use trivia_kind::*;
