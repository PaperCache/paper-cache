use std::hash::Hash;
use rustc_hash::FxHashMap;
use dlv_list::{VecList, Index};
use crate::policy::policy_stack::PolicyStack;

pub struct FifoStack<K>
where
	K: Copy + Eq + Hash,
{
	map: FxHashMap<K, Index<K>>,
	stack: VecList<K>,
}

impl<K> PolicyStack<K> for FifoStack<K>
where
	K: Copy + Eq + Hash,
{
	fn insert(&mut self, key: K) {
		if self.map.contains_key(&key) {
			return self.update(key);
		}

		let index = self.stack.push_front(key);
		self.map.insert(key, index);
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

impl<K> Default for FifoStack<K>
where
	K: Copy + Eq + Hash,
{
	fn default() -> Self {
		FifoStack {
			map: FxHashMap::default(),
			stack: VecList::default(),
		}
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn eviction_order_is_correct() {
		use crate::policy::policy_stack::{PolicyStack, FifoStack};

		let accesses: Vec<u32> = vec![0, 1, 1, 1, 0, 2, 3, 0, 2, 0];
		let mut evictions: Vec<u32> = vec![3, 2, 1, 0];

		let mut stack = FifoStack::<u32>::default();

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
