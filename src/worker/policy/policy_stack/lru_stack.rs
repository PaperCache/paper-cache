use kwik::collections::HashList;

use crate::{
	HashedKey,
	NoHasher,
	object::ObjectSize,
	worker::policy::policy_stack::PolicyStack,
};

#[derive(Default)]
pub struct LruStack {
	stack: HashList<HashedKey, NoHasher>,
}

impl PolicyStack for LruStack {
	fn len(&self) -> usize {
		self.stack.len()
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
		use crate::{
			HashedKey,
			worker::policy::policy_stack::{PolicyStack, LruStack},
		};

		let accesses: Vec<HashedKey> = vec![0, 1, 1, 1, 0, 2, 3, 0, 2, 0];
		let mut evictions: Vec<HashedKey> = vec![0, 2, 3, 1];

		let mut stack = LruStack::default();

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
