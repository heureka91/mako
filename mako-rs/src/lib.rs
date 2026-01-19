//! Mako - Operational Transformation implementation for collaborative text editing
//!
//! This crate provides the core OT algorithms and a Graph-based CRDT for merging
//! concurrent operations.

mod core;

pub use core::{Graph, GraphNode, Op, OpList, PartialFrontier};

// Re-export helper functions for tests
pub use core::{getOpList, getOpListbyVec, oplist_to_string, IntoOp, TestOp};
