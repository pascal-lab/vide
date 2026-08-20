//! Type display of hir-def syntax.
//!
//! Semantic type inference lives in the resident slang elaboration service.
//! This crate pretty-prints lowered hir-def types, expressions, and
//! declarations for hover/render/signature-help.

pub mod db;
pub mod display;
