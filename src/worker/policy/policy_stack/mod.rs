mod lfu_stack;
mod lru_stack;
mod mru_stack;
mod fifo_stack;

use std::hash::{Hash, BuildHasher};

use crate::{
	policy::PaperPolicy,
	worker::policy::policy_stack::{
		lfu_stack::LfuStack,
		lru_stack::LruStack,
		mru_stack::MruStack,
		fifo_stack::FifoStack,
	},
};

pub trait PolicyStack<K, S>
where
	K: Copy + Eq + Hash,
	S: Clone + BuildHasher,
{
	fn with_hasher(hasher: S) -> Self;

	fn len(&self) -> usize;

	fn insert(&mut self, key: K);
	fn update(&mut self, _: K) {}
	fn remove(&mut self, key: K);

	fn clear(&mut self);

	fn pop(&mut self) -> Option<K>;
}

pub enum PolicyStackType<K, S>
where
	K: Copy + Eq + Hash,
	S: Clone + BuildHasher,
{
	Lfu(Box<LfuStack<K, S>>),
	Fifo(Box<FifoStack<K, S>>),
	Lru(Box<LruStack<K, S>>),
	Mru(Box<MruStack<K, S>>),
}

impl<K, S> PolicyStack<K, S> for PolicyStackType<K, S>
where
	K: Copy + Eq + Hash,
	S: Clone + BuildHasher,
{
	fn with_hasher(_hasher: S) -> Self {
		unreachable!();
	}

	fn len(&self) -> usize {
		match self {
			PolicyStackType::Lfu(stack) => stack.len(),
			PolicyStackType::Fifo(stack) => stack.len(),
			PolicyStackType::Lru(stack) => stack.len(),
			PolicyStackType::Mru(stack) => stack.len(),
		}
	}

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

impl<K, S> PolicyStackType<K, S>
where
	K: Copy + Eq + Hash,
	S: Clone + BuildHasher,
{
	#[must_use]
	pub fn init_with_hasher(policy: PaperPolicy, hasher: S) -> Self {
		match policy {
			PaperPolicy::Lfu => PolicyStackType::Lfu(Box::new(LfuStack::with_hasher(hasher))),
			PaperPolicy::Fifo => PolicyStackType::Fifo(Box::new(FifoStack::with_hasher(hasher))),
			PaperPolicy::Lru => PolicyStackType::Lru(Box::new(LruStack::with_hasher(hasher))),
			PaperPolicy::Mru => PolicyStackType::Mru(Box::new(MruStack::with_hasher(hasher))),
		}
	}

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

impl<K, S> PartialEq<PolicyStackType<K, S>> for PaperPolicy
where
	K: Copy + Eq + Hash,
	S: Clone + BuildHasher,
{
	fn eq(&self, policy_type: &PolicyStackType<K, S>) -> bool {
		self.eq(&policy_type)
	}
}

impl<K, S> PartialEq<&PolicyStackType<K, S>> for PaperPolicy
where
	K: Copy + Eq + Hash,
	S: Clone + BuildHasher,
{
	fn eq(&self, policy_type: &&PolicyStackType<K, S>) -> bool {
		matches!(
			(self, policy_type),
			(PaperPolicy::Lfu, PolicyStackType::Lfu(_))
			| (PaperPolicy::Fifo, PolicyStackType::Fifo(_))
			| (PaperPolicy::Lru, PolicyStackType::Lru(_))
			| (PaperPolicy::Mru, PolicyStackType::Mru(_))
		)
	}
}
