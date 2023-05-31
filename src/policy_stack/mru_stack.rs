use std::hash::Hash;
use rustc_hash::FxHashMap;
use dlv_list::{VecList, Index};
use crate::policy_stack::PolicyStack;

pub struct MruStack<K>
where
	K: Eq + Hash + Copy,
{
	map: FxHashMap<K, Index<K>>,
	stack: VecList<K>,
}

impl<K> PolicyStack<K> for MruStack<K>
where
	K: Eq + Hash + Copy,
{
	fn new() -> Self {
		MruStack {
			map: FxHashMap::default(),
			stack: VecList::new(),
		}
	}

	fn insert(&mut self, key: &K) {
		if self.map.contains_key(key) {
			return;
		}

		let index = self.stack.push_front(*key);
		self.map.insert(*key, index);
	}

	fn update(&mut self, key: &K) {
		if let Some(index) = self.map.get(key) {
			if let Some(_) = self.stack.remove(*index) {
				let new_index = self.stack.push_front(*key);
				self.map.insert(*key, new_index);
			}
		}
	}

	fn remove(&mut self, key: &K) {
		if let Some(index) = self.map.get(key) {
			self.stack.remove(*index);
		}
	}

	fn clear(&mut self) {
		self.map.clear();
		self.stack.clear();
	}

	fn get_eviction(&mut self) -> Option<K> {
		match self.stack.pop_front() {
			Some(key) => {
				self.map.remove(&key);
				Some(key)
			},

			None => None,
		}
	}
}
