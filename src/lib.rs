#![feature(variant_count)]

mod cache;
mod expiries;
mod worker;
mod paper_cache;

pub use crate::paper_cache::*;
pub use crate::cache::CacheError;

pub mod stats;
pub use crate::stats::*;

pub mod policy;
pub use crate::policy::*;

mod policy_stack;

mod object;
pub use crate::object::MemSize as ObjectMemSize;
