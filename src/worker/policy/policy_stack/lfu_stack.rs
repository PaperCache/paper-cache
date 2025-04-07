use std::collections::HashMap;
use dlv_list::{VecList, Index};
use kwik::collections::HashList;

use crate::{
	HashedKey,
	NoHasher,
	object::ObjectSize,
	worker::policy::policy_stack::PolicyStack,
};

#[derive(Default)]
pub struct LfuStack {
	index_map: HashMap<HashedKey, Index<CountStack>, NoHasher>,
	count_stacks: VecList<CountStack>,
}

struct CountStack {
	count: u32,
	stack: HashList<HashedKey, NoHasher>,
}

impl PolicyStack for LfuStack {
	fn len(&self) -> usize {
		self.index_map.len()
	}

	fn insert(&mut self, key: HashedKey, _: ObjectSize) {
		if self.index_map.contains_key(&key) {
			return self.update(key);
		}

		if self.count_stacks.front().is_none_or(|count_stack| count_stack.count != 1) {
			self.count_stacks.push_front(CountStack::new(1));
		}

		let count_stack_index = self.count_stacks.front_index().unwrap();
		let count_stack = self.count_stacks.get_mut(count_stack_index).unwrap();

		count_stack.push(key);

		self.index_map.insert(key, count_stack_index);
	}

	fn update(&mut self, key: HashedKey) {
		let Some(count_stack_index) = self.index_map.get(&key) else {
			return;
		};

		let prev_count_stack_index = *count_stack_index;
		let prev_count_stack = self.count_stacks.get_mut(prev_count_stack_index).unwrap();
		let prev_count = prev_count_stack.count;

		prev_count_stack.remove(key);
		let prev_is_empty = prev_count_stack.is_empty();

		if let Some(next_count_stack_index) = self.count_stacks.get_next_index(prev_count_stack_index) {
			let next_count_stack = self.count_stacks.get_mut(next_count_stack_index).unwrap();

			if next_count_stack.count == prev_count + 1 {
				next_count_stack.push(key);
				self.index_map.insert(key, next_count_stack_index);

				if prev_is_empty {
					self.count_stacks.remove(prev_count_stack_index);
				}

				return;
			}
		}

		let mut count_stack = CountStack::new(prev_count + 1);
		count_stack.push(key);

		let count_stack_index = self.count_stacks.insert_after(prev_count_stack_index, count_stack);

		self.index_map.insert(key, count_stack_index);

		if prev_is_empty {
			self.count_stacks.remove(prev_count_stack_index);
		}
	}

	fn remove(&mut self, key: HashedKey) {
		let Some(count_stack_index) = self.index_map.remove(&key) else {
			return;
		};

		let count_stack = self.count_stacks.get_mut(count_stack_index).unwrap();
		count_stack.remove(key);

		if count_stack.is_empty() {
			self.count_stacks.remove(count_stack_index);
		}
	}

	fn clear(&mut self) {
		self.index_map.clear();
		self.count_stacks.clear();
	}

	fn pop(&mut self) -> Option<HashedKey> {
		let count_stack_index = self.count_stacks.front_index()?;
		let count_stack = self.count_stacks.get_mut(count_stack_index)?;

		let key = count_stack.pop();
		self.index_map.remove(&key);

		if count_stack.is_empty() {
			self.count_stacks.remove(count_stack_index);
		}

		Some(key)
	}
}

impl CountStack {
	fn new(count: u32) -> Self {
		CountStack {
			count,
			stack: HashList::with_hasher(NoHasher::default()),
		}
	}

	fn is_empty(&self) -> bool {
		self.stack.is_empty()
	}

	fn push(&mut self, key: HashedKey) {
		self.stack.push_front(key);
	}

	fn pop(&mut self) -> HashedKey {
		self.stack.pop_back().unwrap()
	}

	fn remove(&mut self, key: HashedKey) {
		self.stack.remove(&key).unwrap();
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn eviction_order_is_correct() {
		use crate::{
			HashedKey,
			worker::policy::policy_stack::{PolicyStack, LfuStack},
		};

		let accesses: Vec<HashedKey> = vec![0, 1, 1, 1, 0, 2, 3, 0, 2, 0];
		let mut evictions: Vec<HashedKey> = vec![0, 1, 2, 3];

		let mut stack = LfuStack::default();

		for access in accesses {
			stack.insert(access);
		}

		let mut eviction_count = 0;

		while let Some(key) = stack.pop() {
			match evictions.pop() {
				Some(eviction) => assert_eq!(key, eviction),
				None => unreachable!(),
			}

			eviction_count += 1;
		}

		assert_eq!(eviction_count, 4);
	}
}
