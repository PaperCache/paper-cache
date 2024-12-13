use std::{
	hash::{Hash, BuildHasher},
	collections::HashMap,
};

use dlv_list::{VecList, Index};
use crate::worker::policy::policy_stack::PolicyStack;

pub struct LruStack<K, S>
where
	K: Copy + Eq + Hash,
	S: Clone + BuildHasher,
{
	map: HashMap<K, Index<K>, S>,
	stack: VecList<K>,
}

impl<K, S> PolicyStack<K, S> for LruStack<K, S>
where
	K: Copy + Eq + Hash,
	S: Clone + BuildHasher,
{
	fn with_hasher(hasher: S) -> Self {
		LruStack {
			map: HashMap::with_hasher(hasher),
			stack: VecList::default(),
		}
	}

	fn len(&self) -> usize {
		self.map.len()
	}

	fn insert(&mut self, key: K) {
		if self.map.contains_key(&key) {
			return self.update(key);
		}

		let index = self.stack.push_front(key);
		self.map.insert(key, index);
	}

	fn update(&mut self, key: K) {
		if let Some(index) = self.map.get(&key) {
			if let Some(key) = self.stack.remove(*index) {
				let new_index = self.stack.push_front(key);
				self.map.insert(key, new_index);
			}
		}
	}

	fn remove(&mut self, key: K) {
		if let Some(index) = self.map.remove(&key) {
			self.stack.remove(index);
		}
	}

	fn clear(&mut self) {
		self.map.clear();
		self.stack.clear();
	}

	fn pop(&mut self) -> Option<K> {
		let evicted = self.stack.pop_back();

		if let Some(key) = &evicted {
			self.map.remove(key);
		}

		evicted
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn eviction_order_is_correct() {
		use std::hash::RandomState;
		use crate::worker::policy::policy_stack::{PolicyStack, LruStack};

		let accesses: Vec<u32> = vec![0, 1, 1, 1, 0, 2, 3, 0, 2, 0];
		let mut evictions: Vec<u32> = vec![0, 2, 3, 1];

		let mut stack = LruStack::<u32, RandomState>::with_hasher(RandomState::default());

		for access in accesses {
			stack.insert(access);
		}

		let mut eviction_count = 0;

		while let Some(key) = stack.pop() {
			match evictions.pop() {
				Some(eviction) => assert_eq!(key, eviction),
				None => assert!(false),
			}

			eviction_count += 1;
		}

		assert_eq!(eviction_count, 4);
	}
}
