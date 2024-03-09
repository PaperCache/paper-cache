use std::hash::Hash;
use rustc_hash::FxHashMap;
use dlv_list::{VecList, Index};

use crate::{
	paper_cache::CacheSize,
	object::ObjectSize,
	policy::policy_stack::PolicyStack,
};

pub struct LfuStack<K>
where
	K: Copy + Eq + Hash,
{
	index_map: FxHashMap<K, KeyIndex<K>>,
	count_lists: VecList<CountList<K>>,

	used_size: CacheSize,
}

struct LfuObject<K> {
	key: K,
	size: ObjectSize,
}

struct CountList<K> {
	count: u64,
	list: VecList<LfuObject<K>>,
}

struct KeyIndex<K> {
	count_list_index: Index<CountList<K>>,
	list_index: Index<LfuObject<K>>,
}

impl<K> PolicyStack<K> for LfuStack<K>
where
	K: Copy + Eq + Hash,
{
	fn insert(&mut self, key: K, size: u64) {
		if self.index_map.contains_key(&key) {
			return self.update(key);
		}

		if !self.count_lists.front().is_some_and(|count_list| count_list.count == 1) {
			self.count_lists.push_front(CountList::new(1));
		}

		let count_list_index = self.count_lists.front_index().unwrap();
		let count_list = self.count_lists.get_mut(count_list_index).unwrap();

		let list_index = count_list.push(LfuObject::new(key, size));

		self.index_map.insert(key, KeyIndex::new(
			count_list_index,
			list_index
		));

		self.used_size += size;
	}

	fn update(&mut self, key: K) {
		let Some(key_index) = self.index_map.get(&key) else {
			return;
		};

		let prev_count_list_index = key_index.count_list_index;
		let prev_count_list = self.count_lists.get_mut(prev_count_list_index).unwrap();
		let prev_count = prev_count_list.count;

		let object = prev_count_list.remove(key_index.list_index);
		let prev_is_empty = prev_count_list.is_empty();

		if let Some(next_count_list_index) = self.count_lists.get_next_index(prev_count_list_index) {
			let next_count_list = self.count_lists.get_mut(next_count_list_index).unwrap();

			if next_count_list.count == prev_count + 1 {
				let list_index = next_count_list.push(object);

				self.index_map.insert(key, KeyIndex::new(
					next_count_list_index,
					list_index
				));

				if prev_is_empty {
					self.count_lists.remove(prev_count_list_index);
				}

				return;
			}
		}

		let mut count_list = CountList::<K>::new(prev_count + 1);

		let list_index = count_list.push(object);
		let count_list_index = self.count_lists.insert_after(prev_count_list_index, count_list);

		self.index_map.insert(key, KeyIndex::new(
			count_list_index,
			list_index
		));

		if prev_is_empty {
			self.count_lists.remove(prev_count_list_index);
		}
	}

	fn remove(&mut self, key: K) {
		let Some(key_index) = self.index_map.remove(&key) else {
			return;
		};

		let count_list = self.count_lists.get_mut(key_index.count_list_index).unwrap();
		let object = count_list.remove(key_index.list_index);

		self.used_size -= object.size;

		if count_list.is_empty() {
			self.count_lists.remove(key_index.count_list_index);
		}
	}

	fn clear(&mut self) {
		self.index_map.clear();
		self.count_lists.clear();

		self.used_size = 0;
	}

	fn eviction(&mut self, max_size: CacheSize) -> Option<K> {
		if self.used_size <= max_size {
			return None;
		}

		let count_list_index = self.count_lists.front_index()?;
		let count_list = self.count_lists.get_mut(count_list_index)?;

		let object = count_list.pop();

		self.index_map.remove(&object.key);
		self.used_size -= object.size;

		if count_list.is_empty() {
			self.count_lists.remove(count_list_index);
		}

		Some(object.key)
	}
}

impl<K> CountList<K> {
	fn new(count: u64) -> Self {
		CountList {
			count,
			list: VecList::new(),
		}
	}

	fn is_empty(&self) -> bool {
		self.list.is_empty()
	}

	fn push(&mut self, object: LfuObject<K>) -> Index<LfuObject<K>> {
		self.list.push_front(object)
	}

	fn pop(&mut self) -> LfuObject<K> {
		self.list.pop_back().unwrap()
	}

	fn remove(&mut self, index: Index<LfuObject<K>>) -> LfuObject<K> {
		self.list.remove(index).unwrap()
	}
}

impl<K> LfuObject<K> {
	fn new(key: K, size: ObjectSize) -> Self {
		LfuObject {
			key,
			size,
		}
	}
}

impl<K> Default for LfuStack<K>
where
	K: Copy + Eq + Hash,
{
	fn default() -> Self {
		LfuStack {
			index_map: FxHashMap::default(),
			count_lists: VecList::default(),

			used_size: 0,
		}
	}
}

impl<K> KeyIndex<K> {
	fn new(
		count_list_index: Index<CountList<K>>,
		list_index: Index<LfuObject<K>>,
	) -> Self {
		KeyIndex {
			count_list_index,
			list_index,
		}
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn eviction_order_is_correct() {
		use crate::policy::policy_stack::{PolicyStack, LfuStack};

		let accesses: Vec<u32> = vec![0, 1, 1, 1, 0, 2, 3, 0, 2, 0];
		let mut evictions: Vec<u32> = vec![0, 1, 2, 3];

		let mut stack = LfuStack::<u32>::default();

		for access in accesses {
			stack.insert(access, 1);
		}

		let mut eviction_count = 0;

		while let Some(key) = stack.eviction(0) {
			match evictions.pop() {
				Some(eviction) => assert_eq!(key, eviction),
				None => assert!(false),
			}

			eviction_count += 1;
		}

		assert_eq!(eviction_count, 4);
	}
}
