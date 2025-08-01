/*
 * Copyright (c) Kia Shakiba
 *
 * This source code is licensed under the GNU AGPLv3 license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::collections::HashMap;
use dlv_list::{VecList, Index};
use kwik::collections::HashList;

use crate::{
	HashedKey,
	NoHasher,
	policy::PaperPolicy,
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
	fn is_policy(&self, policy: &PaperPolicy) -> bool {
		matches!(policy, PaperPolicy::Lfu)
	}

	fn len(&self) -> usize {
		self.index_map.len()
	}

	fn contains(&self, key: HashedKey) -> bool {
		self.index_map.contains_key(&key)
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

	fn evict_one(&mut self) -> Option<HashedKey> {
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
		use crate::worker::policy::policy_stack::{PolicyStack, LfuStack};

		let mut stack = LfuStack::default();

		for access in [0, 1, 1, 1, 0, 2, 3, 0, 2, 0] {
			stack.insert(access, 1);
		}

		for eviction in [3, 2, 1, 0] {
			assert_eq!(stack.evict_one(), Some(eviction));
		}

		assert_eq!(stack.evict_one(), None);
	}
}
