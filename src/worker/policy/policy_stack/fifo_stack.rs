/*
 * Copyright (c) Kia Shakiba
 *
 * This source code is licensed under the GNU AGPLv3 license found in the
 * LICENSE file in the root directory of this source tree.
 */

use kwik::collections::HashList;

use crate::{
	HashedKey,
	NoHasher,
	object::ObjectSize,
	policy::PaperPolicy,
	worker::policy::policy_stack::PolicyStack,
};

#[derive(Default)]
pub struct FifoStack {
	stack: HashList<HashedKey, NoHasher>,
}

impl PolicyStack for FifoStack {
	fn is_policy(&self, policy: &PaperPolicy) -> bool {
		matches!(policy, PaperPolicy::Fifo)
	}

	fn len(&self) -> usize {
		self.stack.len()
	}

	fn contains(&self, key: HashedKey) -> bool {
		self.stack.contains(&key)
	}

	fn insert(&mut self, key: HashedKey, _: ObjectSize) {
		if self.stack.contains(&key) {
			return self.update(key);
		}

		self.stack.push_front(key);
	}

	fn remove(&mut self, key: HashedKey) {
		self.stack.remove(&key);
	}

	fn clear(&mut self) {
		self.stack.clear();
	}

	fn evict_one(&mut self) -> Option<HashedKey> {
		self.stack.pop_back()
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn eviction_order_is_correct() {
		use crate::worker::policy::policy_stack::{FifoStack, PolicyStack};
		let mut stack = FifoStack::default();

		for access in [
			0, 1, 1, 1, 0, 2, 3, 0, 2, 0,
		] {
			stack.insert(access, 1);
		}

		for eviction in [0, 1, 2, 3] {
			assert_eq!(stack.evict_one(), Some(eviction));
		}

		assert_eq!(stack.evict_one(), None);
	}
}
