pub use base_db::Cancelled;
pub type Cancellable<T> = Result<T, Cancelled>;

pub mod analysis;
pub mod analysis_host;
pub mod goto_definition;
pub mod navigation_target;
