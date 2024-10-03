use std::hash::{DefaultHasher, Hash, Hasher, BuildHasher};

use crate::{
	policy::PaperPolicy,
	worker::policy::{PolicyStack, PolicyStackType},
};

// the sampling modulus must be a power of 2
const SAMPLING_MODULUS: u64 = 16777216;
const SAMPLING_THRESHOLD: u64 = 16777;

pub enum MiniStackType<K, S>
where
	K: Copy + Eq + Hash,
	S: Clone + BuildHasher,
{
	Lfu(PolicyStackType<K, S>),
	Fifo(PolicyStackType<K, S>),
	Lru(PolicyStackType<K, S>),
	Mru(PolicyStackType<K, S>),
}

impl<K, S> PolicyStack<K, S> for MiniStackType<K, S>
where
	K: Copy + Eq + Hash,
	S: Clone + BuildHasher,
{
	fn with_hasher(_hasher: S) -> Self {
		unreachable!();
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
		let policy_stack = PolicyStackType::<K, S>::init_with_hasher(policy, hasher);

		match policy {
			PaperPolicy::Lfu => MiniStackType::Lfu(policy_stack),
			PaperPolicy::Fifo => MiniStackType::Fifo(policy_stack),
			PaperPolicy::Lru => MiniStackType::Lru(policy_stack),
			PaperPolicy::Mru => MiniStackType::Mru(policy_stack),
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

	hashed & (SAMPLING_MODULUS - 1) < SAMPLING_THRESHOLD
}
