mod lfu_stack;
mod lru_stack;
mod mru_stack;
mod fifo_stack;

use std::hash::Hash;
use crate::policy::PaperPolicy;

pub trait PolicyStack<K>
where
	K: Copy + Eq + Hash,
{
	fn insert(&mut self, key: K);
	fn update(&mut self, _: K) {}
	fn remove(&mut self, key: K);

	fn clear(&mut self);

	fn pop(&mut self) -> Option<K>;
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
	fn insert(&mut self, key: K) {
		match self {
			PolicyStackType::Lfu(stack) => stack.insert(key),
			PolicyStackType::Fifo(stack) => stack.insert(key),
			PolicyStackType::Lru(stack) => stack.insert(key),
			PolicyStackType::Mru(stack) => stack.insert(key),
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

	fn pop(&mut self) -> Option<K> {
		match self {
			PolicyStackType::Lfu(stack) => stack.pop(),
			PolicyStackType::Fifo(stack) => stack.pop(),
			PolicyStackType::Lru(stack) => stack.pop(),
			PolicyStackType::Mru(stack) => stack.pop(),
		}
	}
}

impl<K> PolicyStackType<K>
where
	K: Copy + Eq + Hash,
{
	#[must_use]
	pub fn is_policy(&self, policy: PaperPolicy) -> bool {
		matches!(
			(policy, self),
			(PaperPolicy::Lfu, PolicyStackType::Lfu(_))
			| (PaperPolicy::Fifo, PolicyStackType::Fifo(_))
			| (PaperPolicy::Lru, PolicyStackType::Lru(_))
			| (PaperPolicy::Mru, PolicyStackType::Mru(_))
		)
	}
}

pub use crate::policy::policy_stack::{
	lfu_stack::*,
	lru_stack::*,
	mru_stack::*,
	fifo_stack::*,
};
