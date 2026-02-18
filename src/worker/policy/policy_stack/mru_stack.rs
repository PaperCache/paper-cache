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
pub struct MruStack {
	// we hold the MRU key separately to ensure that it isn't immediately evicted
	// from the cache after it is first set if there are ongoing evictions.
	maybe_mru_key: Option<HashedKey>,
	stack:         HashList<HashedKey, NoHasher>,
}

impl PolicyStack for MruStack {
	fn is_policy(&self, policy: &PaperPolicy) -> bool {
		matches!(policy, PaperPolicy::Mru)
	}

	fn len(&self) -> usize {
		if self.maybe_mru_key.is_none() {
			return 0;
		}

		// return the size of the stack plus 1 to account for the MRU key
		// being handle independently
		self.stack.len() + 1
	}

	fn contains(&self, key: HashedKey) -> bool {
		if self
			.maybe_mru_key
			.is_some_and(|mru_key| mru_key == key)
		{
			return true;
		}

		self.stack.contains(&key)
	}

	fn insert(&mut self, key: HashedKey, _: ObjectSize) {
		if self.stack.contains(&key) {
			return self.update(key);
		}

		if let Some(mru_key) = self.maybe_mru_key {
			// push the previous MRU key to the stack
			self.stack.push_front(mru_key);
		}

		// the key becomes the new MRU key
		self.maybe_mru_key = Some(key);
	}

	fn update(&mut self, key: HashedKey) {
		if self
			.maybe_mru_key
			.is_some_and(|mru_key| mru_key == key)
		{
			// the key is already the most recently used, so do nothing
			return;
		}

		// the key will become the MRU key, so remove it from the stack
		self.stack.remove(&key);

		if let Some(old_mru_key) = self.maybe_mru_key.take() {
			self.stack.push_front(old_mru_key);
		}

		self.maybe_mru_key = Some(key);
	}

	fn remove(&mut self, key: HashedKey) {
		if self
			.maybe_mru_key
			.is_some_and(|mru_key| mru_key == key)
		{
			self.maybe_mru_key = self.stack.pop_front();
			return;
		}

		self.stack.remove(&key);
	}

	fn clear(&mut self) {
		self.maybe_mru_key = None;
		self.stack.clear();
	}

	fn evict_one(&mut self) -> Option<HashedKey> {
		self.stack
			.pop_front()
			.or_else(|| self.maybe_mru_key.take())
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn eviction_order_is_correct() {
		use crate::worker::policy::policy_stack::{MruStack, PolicyStack};

		let mut stack = MruStack::default();

		for access in [0, 1, 0, 2] {
			stack.insert(access, 1);
		}

		// it should skip the immediately most recently accessed key
		assert_eq!(stack.evict_one(), Some(0));

		stack.insert(3, 1);
		assert_eq!(stack.evict_one(), Some(2));

		for access in [3, 0, 2, 0] {
			stack.insert(access, 1);
		}

		for eviction in [2, 3, 1, 0] {
			assert_eq!(stack.evict_one(), Some(eviction));
		}

		assert_eq!(stack.evict_one(), None);
	}
}
