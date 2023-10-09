mod lfu_eviction_map;

use std::mem;

pub use crate::object::eviction_map::lfu_eviction_map::{
	LfuEvictionMap,
};

pub const NUM_POLICIES: usize = mem::variant_count::<EvictionMapPolicy>();

pub trait EvictionMap: Default {
	fn insert(&mut self, _: u64);
	fn update(&mut self) {}
}

pub enum EvictionMapPolicy {
	Lfu(LfuEvictionMap),
}

impl EvictionMap for EvictionMapPolicy {
	fn insert(&mut self, size: u64) {
		match self {
			EvictionMapPolicy::Lfu(eviction_map) => eviction_map.insert(size),
		}
	}

	fn update(&mut self) {
		match self {
			EvictionMapPolicy::Lfu(eviction_map) => eviction_map.update(),
		}
	}
}

impl Default for EvictionMapPolicy {
	fn default() -> Self {
		panic!("Cannot create an eviction map without a policy.");
	}
}
