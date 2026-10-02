//! Duplicate file detection engine: scan a tree, hash candidates, group
//! byte-identical files.

pub mod fsutil;
pub mod group;
pub mod hash;
pub mod human;
pub mod observe;
pub mod pipeline;
pub mod report;
pub mod safety;
pub mod scan;
