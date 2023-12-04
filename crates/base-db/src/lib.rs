pub use salsa::{self, Cancelled};

pub mod change;
pub mod package_graph;
pub mod source_db;
pub mod source_root;

pub trait DbUpcast<T: ?Sized> {
    fn upcast(&self) -> &T;
}
