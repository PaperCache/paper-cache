mod lfu_stack;
mod lru_stack;
mod mru_stack;
mod fifo_stack;

use std::hash::Hash;

use crate::{
	paper_cache::CacheSize,
	object::ObjectSize,
	policy::Policy,
};

pub trait PolicyStack<K>
where
	K: Copy + Eq + Hash,
{
	fn insert(&mut self, key: K, size: ObjectSize);
	fn update(&mut self, _: K) {}
	fn remove(&mut self, key: K);

	fn clear(&mut self);

	fn eviction(&mut self, max_cache_size: CacheSize) -> Option<K>;
}

pub enum PolicyStackType<K>
where
	K: Copy + Eq + Hash,
{
	Lfu(Box<LfuStack<K>>),
	Fifo(Box<FifoStack<K>>),
	Lru(Box<LruStack<K>>),
	Mru(Box<MruStack<K>>),
}

impl<K> PolicyStack<K> for PolicyStackType<K>
where
	K: Copy + Eq + Hash,
{
	fn insert(&mut self, key: K, size: ObjectSize) {
		match self {
			PolicyStackType::Lfu(stack) => stack.insert(key, size),
			PolicyStackType::Fifo(stack) => stack.insert(key, size),
			PolicyStackType::Lru(stack) => stack.insert(key, size),
			PolicyStackType::Mru(stack) => stack.insert(key, size),
		}
	}

	fn update(&mut self, key: K) {
		match self {
			PolicyStackType::Lfu(stack) => stack.update(key),
			PolicyStackType::Fifo(stack) => stack.update(key),
			PolicyStackType::Lru(stack) => stack.update(key),
			PolicyStackType::Mru(stack) => stack.update(key),
		}
	}

	fn remove(&mut self, key: K) {
		match self {
			PolicyStackType::Lfu(stack) => stack.remove(key),
			PolicyStackType::Fifo(stack) => stack.remove(key),
			PolicyStackType::Lru(stack) => stack.remove(key),
			PolicyStackType::Mru(stack) => stack.remove(key),
		}
	}

	fn clear(&mut self) {
		match self {
			PolicyStackType::Lfu(stack) => stack.clear(),
			PolicyStackType::Fifo(stack) => stack.clear(),
			PolicyStackType::Lru(stack) => stack.clear(),
			PolicyStackType::Mru(stack) => stack.clear(),
		}
	}

	fn eviction(&mut self, max_cache_size: CacheSize) -> Option<K> {
		match self {
			PolicyStackType::Lfu(stack) => stack.eviction(max_cache_size),
			PolicyStackType::Fifo(stack) => stack.eviction(max_cache_size),
			PolicyStackType::Lru(stack) => stack.eviction(max_cache_size),
			PolicyStackType::Mru(stack) => stack.eviction(max_cache_size),
		}
	}
}

impl<K> PolicyStackType<K>
where
	K: Copy + Eq + Hash,
{
	pub fn is_policy(&self, policy: Policy) -> bool {
		matches!(
			(policy, self),
			(Policy::Lfu, PolicyStackType::Lfu(_))
			| (Policy::Fifo, PolicyStackType::Fifo(_))
			| (Policy::Lru, PolicyStackType::Lru(_))
			| (Policy::Mru, PolicyStackType::Mru(_))
		)
	}
}

pub use crate::policy::policy_stack::{
	lfu_stack::*,
	lru_stack::*,
	mru_stack::*,
	fifo_stack::*,
};
