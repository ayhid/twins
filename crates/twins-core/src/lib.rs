//! Duplicate file detection engine: scan a tree, hash candidates, group
//! byte-identical files.

pub mod fsutil;
pub mod human;
pub mod safety;
