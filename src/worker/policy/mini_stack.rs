use std::hash::{DefaultHasher, Hash, Hasher};

use crate::{
	policy::PaperPolicy,
	worker::policy::{PolicyStack, PolicyStackType},
};

// the sampling modulus must be a power of 2
const SAMPLING_MODULUS: u64 = 16777216;
const SAMPLING_THRESHOLD: u64 = 16777;

pub enum MiniStackType<K>
where
	K: Copy + Eq + Hash,
{
	Lfu(PolicyStackType<K>),
	Fifo(PolicyStackType<K>),
	Lru(PolicyStackType<K>),
	Mru(PolicyStackType<K>),
}

impl<K> PolicyStack<K> for MiniStackType<K>
where
	K: Copy + Eq + Hash,
{
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

impl<K> MiniStackType<K>
where
	K: Copy + Eq + Hash,
{
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

impl<K> From<PaperPolicy> for MiniStackType<K>
where
	K: Copy + Eq + Hash,
{
	fn from(policy: PaperPolicy) -> Self {
		(&policy).into()
	}
}

impl<K> From<&PaperPolicy> for MiniStackType<K>
where
	K: Copy + Eq + Hash,
{
	fn from(policy: &PaperPolicy) -> Self {
		match policy {
			PaperPolicy::Lfu => MiniStackType::Lfu(policy.into()),
			PaperPolicy::Fifo => MiniStackType::Fifo(policy.into()),
			PaperPolicy::Lru => MiniStackType::Lru(policy.into()),
			PaperPolicy::Mru => MiniStackType::Mru(policy.into()),
		}
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
