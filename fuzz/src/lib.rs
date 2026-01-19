//! Fuzz testing library for comparing Mako and Fugue-Simple
//!
//! This crate provides property-based testing to verify that Mako's
//! OT implementation produces identical results to the Fugue-Simple
//! reference implementation.

pub mod actions;
pub mod harness;

pub use actions::FuzzAction;
pub use harness::{FugueReplica, MakoReplica, MakoUpdate, TestHarness};
