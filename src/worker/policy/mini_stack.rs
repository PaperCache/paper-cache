use std::hash::{DefaultHasher, Hash, Hasher, BuildHasher};

use crate::{
	policy::PaperPolicy,
	worker::policy::policy_stack::{
		PolicyStack,
		LfuStack,
		FifoStack,
		LruStack,
		MruStack,
	},
};

// the sampling modulus must be a power of 2
const SAMPLING_MODULUS: u64 = 16777216;
const SAMPLING_THRESHOLD: u64 = 16777;

pub enum MiniStackType<K, S>
where
	K: Copy + Eq + Hash,
	S: Clone + BuildHasher,
{
	Lfu(Box<LfuStack<K, S>>),
	Fifo(Box<FifoStack<K, S>>),
	Lru(Box<LruStack<K, S>>),
	Mru(Box<MruStack<K, S>>),
}

impl<K, S> PolicyStack<K, S> for MiniStackType<K, S>
where
	K: Copy + Eq + Hash,
	S: Clone + BuildHasher,
{
	fn with_hasher(_hasher: S) -> Self {
		unimplemented!();
	}

	fn insert(&mut self, key: K) {
		if !should_sample(key) {
			return;
		}

		match self {
			MiniStackType::Lfu(stack) => stack.insert(key),
			MiniStackType::Fifo(stack) => stack.insert(key),
			MiniStackType::Lru(stack) => stack.insert(key),
			MiniStackType::Mru(stack) => stack.insert(key),
		}
	}

	fn update(&mut self, key: K) {
		match self {
			MiniStackType::Lfu(stack) => stack.update(key),
			MiniStackType::Fifo(stack) => stack.update(key),
			MiniStackType::Lru(stack) => stack.update(key),
			MiniStackType::Mru(stack) => stack.update(key),
		}
	}

	fn remove(&mut self, key: K) {
		match self {
			MiniStackType::Lfu(stack) => stack.remove(key),
			MiniStackType::Fifo(stack) => stack.remove(key),
			MiniStackType::Lru(stack) => stack.remove(key),
			MiniStackType::Mru(stack) => stack.remove(key),
		}
	}

	fn clear(&mut self) {
		match self {
			MiniStackType::Lfu(stack) => stack.clear(),
			MiniStackType::Fifo(stack) => stack.clear(),
			MiniStackType::Lru(stack) => stack.clear(),
			MiniStackType::Mru(stack) => stack.clear(),
		}
	}

	fn pop(&mut self) -> Option<K> {
		match self {
			MiniStackType::Lfu(stack) => stack.pop(),
			MiniStackType::Fifo(stack) => stack.pop(),
			MiniStackType::Lru(stack) => stack.pop(),
			MiniStackType::Mru(stack) => stack.pop(),
		}
	}
}

impl<K, S> MiniStackType<K, S>
where
	K: Copy + Eq + Hash,
	S: Clone + BuildHasher,
{
	#[must_use]
	pub fn init_with_hasher(policy: PaperPolicy, hasher: S) -> Self {
		match policy {
			PaperPolicy::Lfu => MiniStackType::Lfu(Box::new(LfuStack::with_hasher(hasher))),
			PaperPolicy::Fifo => MiniStackType::Fifo(Box::new(FifoStack::with_hasher(hasher))),
			PaperPolicy::Lru => MiniStackType::Lru(Box::new(LruStack::with_hasher(hasher))),
			PaperPolicy::Mru => MiniStackType::Mru(Box::new(MruStack::with_hasher(hasher))),
		}
	}

	#[must_use]
	pub fn is_policy(&self, policy: PaperPolicy) -> bool {
		matches!(
			(policy, self),
			(PaperPolicy::Lfu, MiniStackType::Lfu(_))
			| (PaperPolicy::Fifo, MiniStackType::Fifo(_))
			| (PaperPolicy::Lru, MiniStackType::Lru(_))
			| (PaperPolicy::Mru, MiniStackType::Mru(_))
		)
	}
}

fn should_sample<K>(key: K) -> bool
where
	K: Hash,
{
	let mut s = DefaultHasher::new();
	key.hash(&mut s);
	let hashed = s.finish();

	// this optimization only works if the sampling modulus is a power of 2
	hashed & (SAMPLING_MODULUS - 1) < SAMPLING_THRESHOLD
}
