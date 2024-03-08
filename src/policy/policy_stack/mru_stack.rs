use std::hash::Hash;
use rustc_hash::FxHashMap;
use dlv_list::{VecList, Index};

use crate::{
	paper_cache::CacheSize,
	object::ObjectSize,
	policy::policy_stack::PolicyStack,
};

pub struct MruStack<K>
where
	K: Copy + Eq + Hash,
{
	map: FxHashMap<K, Index<MruObject<K>>>,
	stack: VecList<MruObject<K>>,

	used_size: CacheSize,
}

struct MruObject<K> {
	key: K,
	size: ObjectSize,
}

impl<K> PolicyStack<K> for MruStack<K>
where
	K: Copy + Eq + Hash,
{
	fn insert(&mut self, key: K, size: u64) {
		if self.map.contains_key(&key) {
			return self.update(key);
		}

		let index = self.stack.push_front(MruObject::new(key, size));
		self.map.insert(key, index);

		self.used_size += size;
	}

	fn update(&mut self, key: K) {
		if let Some(index) = self.map.get(&key) {
			if let Some(object) = self.stack.remove(*index) {
				let new_index = self.stack.push_front(object);
				self.map.insert(key, new_index);
			}
		}
	}

	fn remove(&mut self, key: K) {
		if let Some(index) = self.map.remove(&key) {
			if let Some(object) = self.stack.remove(index) {
				self.used_size -= object.size;
			}
		}
	}

	fn clear(&mut self) {
		self.map.clear();
		self.stack.clear();

		self.used_size = 0;
	}

	fn eviction(&mut self, max_size: CacheSize) -> Option<K> {
		if self.used_size <= max_size {
			return None;
		}

		let evicted = self.stack.pop_front();

		if let Some(object) = &evicted {
			self.map.remove(&object.key);
			self.used_size -= object.size;
		}

		evicted.map(|object| object.key)
	}
}

impl<K> MruObject<K> {
	fn new(key: K, size: ObjectSize) -> Self {
		MruObject {
			key,
			size,
		}
	}
}

impl<K> Default for MruStack<K>
where
	K: Copy + Eq + Hash,
{
	fn default() -> Self {
		MruStack {
			map: FxHashMap::default(),
			stack: VecList::default(),

			used_size: 0,
		}
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn eviction_order_is_correct() {
		use crate::policy::policy_stack::{PolicyStack, MruStack};

		let accesses: Vec<u32> = vec![0, 1, 1, 1, 0, 2, 3, 0, 2, 0];
		let mut evictions: Vec<u32> = vec![1, 3, 2, 0];

		let mut stack = MruStack::<u32>::default();

		for access in accesses {
			stack.insert(access, 1);
		}

		let mut eviction_count = 0;

		while let Some(key) = stack.eviction(1) {
			match evictions.pop() {
				Some(eviction) => assert_eq!(key, eviction),
				None => assert!(false),
			}

			eviction_count += 1;
		}

		assert_eq!(eviction_count, 4);
	}
}
