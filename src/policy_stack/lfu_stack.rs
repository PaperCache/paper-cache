use std::hash::Hash;
use rustc_hash::FxHashMap;
use dlv_list::{VecList, Index};
use crate::policy_stack::PolicyStack;

pub struct LfuStack<K>
where
	K: Eq + Hash + Clone,
{
	count_map: FxHashMap<K, u64>,
	index_map: FxHashMap<K, Index<K>>,

	counts: Vec<VecList<K>>,
}

impl<K> PolicyStack<K> for LfuStack<K>
where
	K: Eq + Hash + Clone,
{
	fn new() -> Self {
		LfuStack {
			count_map: FxHashMap::default(),
			index_map: FxHashMap::default(),

			counts: Vec::new(),
		}
	}

	fn insert(&mut self, key: &K) {
		if self.count_map.contains_key(key) {
			return self.update(key);
		}

		self.init_counts(1);

		let index = self.counts[0].push_front(key.clone());

		self.count_map.insert(key.clone(), 0);
		self.index_map.insert(key.clone(), index);
	}

	fn update(&mut self, key: &K) {
		if let (Some(count), Some(index)) = (
			self.count_map.get(key),
			self.index_map.get(key),
		) {
			if self.counts[*count as usize].remove(*index).is_some() {
				let count = *count + 1;

				self.init_counts(count);
				self.count_map.insert(key.clone(), count);

				let new_index = self.counts[count as usize].push_front(key.clone());
				self.index_map.insert(key.clone(), new_index);
			}
		}
	}

	fn remove(&mut self, key: &K) {
		if let (Some(count), Some(index)) = (
			self.count_map.get(key),
			self.index_map.get(key),
		) {
			self.counts[*count as usize].remove(*index);
		}

		self.count_map.remove(key);
		self.index_map.remove(key);
	}

	fn clear(&mut self) {
		self.count_map.clear();
		self.index_map.clear();

		self.counts.clear();
	}

	fn get_eviction(&mut self) -> Option<K> {
		let mut count_index: usize = 0;

		while count_index < self.counts.len() {
			if let Some(key) = self.counts[count_index].pop_back() {
				self.count_map.remove(&key);
				self.index_map.remove(&key);

				return Some(key);
			}

			count_index += 1;
		}

		None
	}
}

impl<K> LfuStack<K>
where
	K: Eq + Hash + Clone,
{
	fn init_counts(&mut self, min_count: u64) {
		while self.counts.len() <= min_count as usize {
			self.counts.push(VecList::new());
		}
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn eviction_order_is_correct() {
		use crate::policy_stack::{PolicyStack, LfuStack};

		let accesses: Vec<u32> = vec![0, 1, 1, 1, 0, 2, 3, 0, 2, 0];
		let mut evictions: Vec<u32> = vec![0, 1, 2, 3];

		let mut stack = LfuStack::<u32>::new();

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
