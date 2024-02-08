use std::{
	rc::Rc,
	hash::Hash,
};

use crate::policy::policy_stack::{
	PolicyStack,
	LfuStack,
	LruStack,
	MruStack,
	FifoStack,
};

pub enum PolicyType<K>
where
	K: Eq + Hash,
{
	Lfu(Box<LfuStack<K>>),
	Fifo(Box<FifoStack<K>>),
	Lru(Box<LruStack<K>>),
	Mru(Box<MruStack<K>>),
}

impl<K> PolicyStack<K> for PolicyType<K>
where
	K: Eq + Hash,
{
	fn insert(&mut self, key: &Rc<K>) {
		match self {
			PolicyType::Lfu(stack) => stack.insert(key),
			PolicyType::Fifo(stack) => stack.insert(key),
			PolicyType::Lru(stack) => stack.insert(key),
			PolicyType::Mru(stack) => stack.insert(key),
		}
	}

	fn update(&mut self, key: &Rc<K>) {
		match self {
			PolicyType::Lfu(stack) => stack.update(key),
			PolicyType::Fifo(stack) => stack.update(key),
			PolicyType::Lru(stack) => stack.update(key),
			PolicyType::Mru(stack) => stack.update(key),
		}
	}

	fn remove(&mut self, key: &K) {
		match self {
			PolicyType::Lfu(stack) => stack.remove(key),
			PolicyType::Fifo(stack) => stack.remove(key),
			PolicyType::Lru(stack) => stack.remove(key),
			PolicyType::Mru(stack) => stack.remove(key),
		}
	}

	fn clear(&mut self) {
		match self {
			PolicyType::Lfu(stack) => stack.clear(),
			PolicyType::Fifo(stack) => stack.clear(),
			PolicyType::Lru(stack) => stack.clear(),
			PolicyType::Mru(stack) => stack.clear(),
		}
	}

	fn get_eviction(&mut self) -> Option<Rc<K>> {
		match self {
			PolicyType::Lfu(stack) => stack.get_eviction(),
			PolicyType::Fifo(stack) => stack.get_eviction(),
			PolicyType::Lru(stack) => stack.get_eviction(),
			PolicyType::Mru(stack) => stack.get_eviction(),
		}
	}
}
