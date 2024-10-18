use std::{
	hash::{Hash, BuildHasher},
	collections::HashMap,
};

use dlv_list::{VecList, Index};
use crate::worker::policy::policy_stack::PolicyStack;

pub struct LfuStack<K, S>
where
	K: Copy + Eq + Hash,
	S: Clone + BuildHasher,
{
	index_map: HashMap<K, KeyIndex<K>, S>,
	count_lists: VecList<CountList<K>>,
}

struct CountList<K> {
	count: u32,
	list: VecList<K>,
}

struct KeyIndex<K> {
	count_list_index: Index<CountList<K>>,
	list_index: Index<K>,
}

impl<K, S> PolicyStack<K, S> for LfuStack<K, S>
where
	K: Copy + Eq + Hash,
	S: Clone + BuildHasher,
{
	fn with_hasher(hasher: S) -> Self {
		LfuStack {
			index_map: HashMap::with_hasher(hasher),
			count_lists: VecList::default(),
		}
	}

	fn insert(&mut self, key: K) {
		if self.index_map.contains_key(&key) {
			return self.update(key);
		}

		if self.count_lists.front().is_none_or(|count_list| count_list.count != 1) {
			self.count_lists.push_front(CountList::new(1));
		}

		let count_list_index = self.count_lists.front_index().unwrap();
		let count_list = self.count_lists.get_mut(count_list_index).unwrap();

		let list_index = count_list.push(key);

		self.index_map.insert(key, KeyIndex::new(
			count_list_index,
			list_index,
		));
	}

	fn update(&mut self, key: K) {
		let Some(key_index) = self.index_map.get(&key) else {
			return;
		};

		let prev_count_list_index = key_index.count_list_index;
		let prev_count_list = self.count_lists.get_mut(prev_count_list_index).unwrap();
		let prev_count = prev_count_list.count;

		prev_count_list.remove(key_index.list_index);
		let prev_is_empty = prev_count_list.is_empty();

		if let Some(next_count_list_index) = self.count_lists.get_next_index(prev_count_list_index) {
			let next_count_list = self.count_lists.get_mut(next_count_list_index).unwrap();

			if next_count_list.count == prev_count + 1 {
				let list_index = next_count_list.push(key);

				self.index_map.insert(key, KeyIndex::new(
					next_count_list_index,
					list_index,
				));

				if prev_is_empty {
					self.count_lists.remove(prev_count_list_index);
				}

				return;
			}
		}

		let mut count_list = CountList::<K>::new(prev_count + 1);

		let list_index = count_list.push(key);
		let count_list_index = self.count_lists.insert_after(prev_count_list_index, count_list);

		self.index_map.insert(key, KeyIndex::new(
			count_list_index,
			list_index,
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
		count_list.remove(key_index.list_index);

		if count_list.is_empty() {
			self.count_lists.remove(key_index.count_list_index);
		}
	}

	fn clear(&mut self) {
		self.index_map.clear();
		self.count_lists.clear();
	}

	fn pop(&mut self) -> Option<K> {
		let count_list_index = self.count_lists.front_index()?;
		let count_list = self.count_lists.get_mut(count_list_index)?;

		let key = count_list.pop();
		self.index_map.remove(&key);

		if count_list.is_empty() {
			self.count_lists.remove(count_list_index);
		}

		Some(key)
	}
}

impl<K> CountList<K> {
	fn new(count: u32) -> Self {
		CountList {
			count,
			list: VecList::new(),
		}
	}

	fn is_empty(&self) -> bool {
		self.list.is_empty()
	}

	fn push(&mut self, key: K) -> Index<K> {
		self.list.push_front(key)
	}

	fn pop(&mut self) -> K {
		self.list.pop_back().unwrap()
	}

	fn remove(&mut self, index: Index<K>) {
		self.list.remove(index).unwrap();
	}
}

impl<K> KeyIndex<K> {
	fn new(
		count_list_index: Index<CountList<K>>,
		list_index: Index<K>,
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
		use std::hash::RandomState;
		use crate::worker::policy::policy_stack::{PolicyStack, LfuStack};

		let accesses: Vec<u32> = vec![0, 1, 1, 1, 0, 2, 3, 0, 2, 0];
		let mut evictions: Vec<u32> = vec![0, 1, 2, 3];

		let mut stack = LfuStack::<u32, RandomState>::with_hasher(RandomState::default());

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
