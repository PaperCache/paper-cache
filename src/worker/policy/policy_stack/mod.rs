mod lfu_stack;
mod lru_stack;
mod mru_stack;
mod fifo_stack;

use crate::{
	cache::HashedKey,
	policy::PaperPolicy,
	worker::policy::policy_stack::{
		lfu_stack::LfuStack,
		lru_stack::LruStack,
		mru_stack::MruStack,
		fifo_stack::FifoStack,
	},
};

pub trait PolicyStack
where
	Self: Default,
{
	fn len(&self) -> usize;

	fn insert(&mut self, key: HashedKey);
	fn update(&mut self, _: HashedKey) {}
	fn remove(&mut self, key: HashedKey);

	fn clear(&mut self);

	fn pop(&mut self) -> Option<HashedKey>;
}

pub enum PolicyStackType {
	Lfu(Box<LfuStack>),
	Fifo(Box<FifoStack>),
	Lru(Box<LruStack>),
	Mru(Box<MruStack>),
}

impl PolicyStack for PolicyStackType {
	fn len(&self) -> usize {
		match self {
			PolicyStackType::Lfu(stack) => stack.len(),
			PolicyStackType::Fifo(stack) => stack.len(),
			PolicyStackType::Lru(stack) => stack.len(),
			PolicyStackType::Mru(stack) => stack.len(),
		}
	}

	fn insert(&mut self, key: HashedKey) {
		match self {
			PolicyStackType::Lfu(stack) => stack.insert(key),
			PolicyStackType::Fifo(stack) => stack.insert(key),
			PolicyStackType::Lru(stack) => stack.insert(key),
			PolicyStackType::Mru(stack) => stack.insert(key),
		}
	}

	fn update(&mut self, key: HashedKey) {
		match self {
			PolicyStackType::Lfu(stack) => stack.update(key),
			PolicyStackType::Fifo(stack) => stack.update(key),
			PolicyStackType::Lru(stack) => stack.update(key),
			PolicyStackType::Mru(stack) => stack.update(key),
		}
	}

	fn remove(&mut self, key: HashedKey) {
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

	fn pop(&mut self) -> Option<HashedKey> {
		match self {
			PolicyStackType::Lfu(stack) => stack.pop(),
			PolicyStackType::Fifo(stack) => stack.pop(),
			PolicyStackType::Lru(stack) => stack.pop(),
			PolicyStackType::Mru(stack) => stack.pop(),
		}
	}
}

impl PolicyStackType {
	#[must_use]
	pub fn new(policy: PaperPolicy) -> Self {
		match policy {
			PaperPolicy::Lfu => PolicyStackType::Lfu(Box::default()),
			PaperPolicy::Fifo => PolicyStackType::Fifo(Box::default()),
			PaperPolicy::Lru => PolicyStackType::Lru(Box::default()),
			PaperPolicy::Mru => PolicyStackType::Mru(Box::default()),
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

impl Default for PolicyStackType {
	fn default() -> Self {
		unreachable!();
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
