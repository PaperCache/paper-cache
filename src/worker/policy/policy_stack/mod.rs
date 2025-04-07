mod lfu_stack;
mod fifo_stack;
mod lru_stack;
mod mru_stack;
mod two_q_stack;

use crate::{
	CacheSize,
	HashedKey,
	policy::PaperPolicy,
	object::ObjectSize,
	worker::policy::policy_stack::{
		lfu_stack::LfuStack,
		lru_stack::LruStack,
		mru_stack::MruStack,
		fifo_stack::FifoStack,
		two_q_stack::TwoQStack,
	},
};

pub trait PolicyStack {
	fn len(&self) -> usize;

	fn insert(&mut self, key: HashedKey, size: ObjectSize);
	fn update(&mut self, _key: HashedKey) {}
	fn remove(&mut self, key: HashedKey);

	fn resize(&mut self, _size: CacheSize) {}
	fn clear(&mut self);

	fn pop(&mut self) -> Option<HashedKey>;
}

pub enum PolicyStackType {
	Lfu(Box<LfuStack>),
	Fifo(Box<FifoStack>),
	Lru(Box<LruStack>),
	Mru(Box<MruStack>),
	TwoQ(Box<TwoQStack>),
}

impl PolicyStack for PolicyStackType {
	fn len(&self) -> usize {
		match self {
			PolicyStackType::Lfu(stack) => stack.len(),
			PolicyStackType::Fifo(stack) => stack.len(),
			PolicyStackType::Lru(stack) => stack.len(),
			PolicyStackType::Mru(stack) => stack.len(),
			PolicyStackType::TwoQ(stack) => stack.len(),
		}
	}

	fn insert(&mut self, key: HashedKey, size: ObjectSize) {
		match self {
			PolicyStackType::Lfu(stack) => stack.insert(key, size),
			PolicyStackType::Fifo(stack) => stack.insert(key, size),
			PolicyStackType::Lru(stack) => stack.insert(key, size),
			PolicyStackType::Mru(stack) => stack.insert(key, size),
			PolicyStackType::TwoQ(stack) => stack.insert(key, size),
		}
	}

	fn update(&mut self, key: HashedKey) {
		match self {
			PolicyStackType::Lfu(stack) => stack.update(key),
			PolicyStackType::Fifo(stack) => stack.update(key),
			PolicyStackType::Lru(stack) => stack.update(key),
			PolicyStackType::Mru(stack) => stack.update(key),
			PolicyStackType::TwoQ(stack) => stack.update(key),
		}
	}

	fn remove(&mut self, key: HashedKey) {
		match self {
			PolicyStackType::Lfu(stack) => stack.remove(key),
			PolicyStackType::Fifo(stack) => stack.remove(key),
			PolicyStackType::Lru(stack) => stack.remove(key),
			PolicyStackType::Mru(stack) => stack.remove(key),
			PolicyStackType::TwoQ(stack) => stack.remove(key),
		}
	}

	fn clear(&mut self) {
		match self {
			PolicyStackType::Lfu(stack) => stack.clear(),
			PolicyStackType::Fifo(stack) => stack.clear(),
			PolicyStackType::Lru(stack) => stack.clear(),
			PolicyStackType::Mru(stack) => stack.clear(),
			PolicyStackType::TwoQ(stack) => stack.clear(),
		}
	}

	fn pop(&mut self) -> Option<HashedKey> {
		match self {
			PolicyStackType::Lfu(stack) => stack.pop(),
			PolicyStackType::Fifo(stack) => stack.pop(),
			PolicyStackType::Lru(stack) => stack.pop(),
			PolicyStackType::Mru(stack) => stack.pop(),
			PolicyStackType::TwoQ(stack) => stack.pop(),
		}
	}
}

impl PolicyStackType {
	#[must_use]
	pub fn new(policy: PaperPolicy, max_size: CacheSize) -> Self {
		match policy {
			PaperPolicy::Lfu => PolicyStackType::Lfu(Box::default()),
			PaperPolicy::Fifo => PolicyStackType::Fifo(Box::default()),
			PaperPolicy::Lru => PolicyStackType::Lru(Box::default()),
			PaperPolicy::Mru => PolicyStackType::Mru(Box::default()),

			PaperPolicy::TwoQ(k_in, k_out) => {
				let stack = TwoQStack::new(k_in, k_out, max_size);
				PolicyStackType::TwoQ(Box::new(stack))
			}
		}
	}

	#[must_use]
	pub fn is_policy(&self, policy: PaperPolicy) -> bool {
		self.eq(&policy)
	}
}

impl PartialEq<PaperPolicy> for PolicyStackType {
	fn eq(&self, policy: &PaperPolicy) -> bool {
		policy.eq(self)
	}
}

impl PartialEq<PaperPolicy> for &PolicyStackType {
	fn eq(&self, policy: &PaperPolicy) -> bool {
		policy.eq(self)
	}
}

impl PartialEq<PolicyStackType> for PaperPolicy {
	fn eq(&self, policy_type: &PolicyStackType) -> bool {
		self.eq(&policy_type)
	}
}

impl PartialEq<&PolicyStackType> for PaperPolicy {
	fn eq(&self, policy_type: &&PolicyStackType) -> bool {
		matches!(
			(self, policy_type),
			(PaperPolicy::Lfu, PolicyStackType::Lfu(_))
			| (PaperPolicy::Fifo, PolicyStackType::Fifo(_))
			| (PaperPolicy::Lru, PolicyStackType::Lru(_))
			| (PaperPolicy::Mru, PolicyStackType::Mru(_))
		)
	}
}
