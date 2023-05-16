#![feature(variant_count)]

mod paper_cache;
pub use paper_cache::*;

pub mod command;
pub use command::*;

pub mod cache_error;
pub use cache_error::*;

mod object;
mod policy;
mod policy_stack;
