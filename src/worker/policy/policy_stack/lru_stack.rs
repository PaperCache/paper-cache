use kwik::collections::HashList;

use crate::{
	HashedKey,
	NoHasher,
	policy::PaperPolicy,
	object::ObjectSize,
	worker::policy::policy_stack::PolicyStack,
};

#[derive(Default)]
pub struct LruStack {
	stack: HashList<HashedKey, NoHasher>,
}

impl PolicyStack for LruStack {
	fn is_policy(&self, policy: &PaperPolicy) -> bool {
		matches!(policy, PaperPolicy::Lru)
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

	fn update(&mut self, key: HashedKey) {
		self.stack.move_front(&key);
	}

	fn remove(&mut self, key: HashedKey) {
		self.stack.remove(&key);
	}

	fn clear(&mut self) {
		self.stack.clear();
	}

	fn pop(&mut self) -> Option<HashedKey> {
		self.stack.pop_back()
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn eviction_order_is_correct() {
		use crate::worker::policy::policy_stack::{PolicyStack, LruStack};

		let mut stack = LruStack::default();

		for access in [0, 1, 1, 1, 0, 2, 3, 0, 2, 0] {
			stack.insert(access, 1);
		}

		for eviction in [1, 3, 2, 0] {
			assert_eq!(stack.pop(), Some(eviction));
		}

		assert_eq!(stack.pop(), None);
	}
}
