use std::hash::Hash;
use rustc_hash::FxHashMap;
use dlv_list::{VecList, Index};
use crate::policy_stack::PolicyStack;

pub struct MruStack<K>
where
	K: Eq + Hash + Clone,
{
	map: FxHashMap<K, Index<K>>,
	stack: VecList<K>,
}

impl<K> PolicyStack<K> for MruStack<K>
where
	K: Eq + Hash + Clone,
{
	fn new() -> Self {
		MruStack {
			map: FxHashMap::default(),
			stack: VecList::new(),
		}
	}

	fn insert(&mut self, key: &K) {
		if self.map.contains_key(key) {
			return self.update(key);
		}

		let index = self.stack.push_front(key.clone());
		self.map.insert(key.clone(), index);
	}

	fn update(&mut self, key: &K) {
		if let Some(index) = self.map.get(key) {
			if self.stack.remove(*index).is_some() {
				let new_index = self.stack.push_front(key.clone());
				self.map.insert(key.clone(), new_index);
			}
		}
	}

	fn remove(&mut self, key: &K) {
		if let Some(index) = self.map.remove(key) {
			self.stack.remove(index);
		}
	}

	fn clear(&mut self) {
		self.map.clear();
		self.stack.clear();
	}

	fn get_eviction(&mut self) -> Option<K> {
		let evict_key = self.stack.pop_front();

		if let Some(key) = &evict_key {
			self.map.remove(key);
		}

		evict_key
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn eviction_order_is_correct() {
		use crate::policy_stack::{PolicyStack, MruStack};

		let accesses: Vec<u32> = vec![0, 1, 1, 1, 0, 2, 3, 0, 2, 0];
		let mut evictions: Vec<u32> = vec![1, 3, 2, 0];

		let mut stack = MruStack::<u32>::new();

		for access in &accesses {
			stack.insert(access);
		}

		let mut eviction_count = 0;

		while let Some(key) = stack.get_eviction() {
			match evictions.pop() {
				Some(eviction) => assert_eq!(key, eviction),
				None => assert!(false),
			}

			eviction_count += 1;
		}

		assert_eq!(eviction_count, 4);
	}
}
