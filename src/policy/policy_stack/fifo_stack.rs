use std::{
	rc::Rc,
	hash::Hash,
};

use rustc_hash::FxHashMap;
use dlv_list::{VecList, Index};
use crate::policy::policy_stack::PolicyStack;

pub struct FifoStack<K>
where
	K: Eq + Hash,
{
	map: FxHashMap<Rc<K>, Index<Rc<K>>>,
	stack: VecList<Rc<K>>,
}

impl<K> PolicyStack<K> for FifoStack<K>
where
	K: Eq + Hash,
{
	fn insert(&mut self, key: &Rc<K>) {
		if self.map.contains_key(key) {
			return self.update(key);
		}

		let index = self.stack.push_front(Rc::clone(key));
		self.map.insert(Rc::clone(key), index);
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

	fn get_eviction(&mut self) -> Option<Rc<K>> {
		let evict_key = self.stack.pop_back();

		if let Some(key) = &evict_key {
			self.map.remove(key);
		}

		evict_key
	}
}

impl<K> Default for FifoStack<K>
where
	K: Eq + Hash,
{
	fn default() -> Self {
		FifoStack {
			map: FxHashMap::default(),
			stack: VecList::new(),
		}
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn eviction_order_is_correct() {
		use std::rc::Rc;
		use crate::policy::policy_stack::{PolicyStack, FifoStack};

		let accesses: Vec<u32> = vec![0, 1, 1, 1, 0, 2, 3, 0, 2, 0];
		let mut evictions: Vec<u32> = vec![3, 2, 1, 0];

		let mut stack = FifoStack::<u32>::default();

		for access in &accesses {
			stack.insert(&Rc::new(*access));
		}

		let mut eviction_count = 0;

		while let Some(key) = stack.get_eviction() {
			match evictions.pop() {
				Some(eviction) => assert_eq!(key, Rc::new(eviction)),
				None => assert!(false),
			}

			eviction_count += 1;
		}

		assert_eq!(eviction_count, 4);
	}
}
