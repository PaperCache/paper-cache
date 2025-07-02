/*
 * Copyright (c) Kia Shakiba
 *
 * This source code is licensed under the GNU AGPLv3 license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::{
	borrow::Borrow,
	hash::{Hash, Hasher},
};

use kwik::collections::HashList;

use crate::{
	HashedKey,
	NoHasher,
	policy::PaperPolicy,
	object::ObjectSize,
	worker::policy::policy_stack::PolicyStack,
};

#[derive(Default)]
pub struct SieveStack {
	stack: HashList<Object, NoHasher>,
	hand: Option<HashedKey>,
}

struct Object {
	key: HashedKey,
	visited: bool,
}

impl PolicyStack for SieveStack {
	fn is_policy(&self, policy: &PaperPolicy) -> bool {
		matches!(policy, PaperPolicy::Sieve)
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

		self.stack.push_front(Object::new(key));
	}

	fn update(&mut self, key: HashedKey) {
		self.stack.update(&key, |object| {
			object.visited = true;
		});
	}

	fn remove(&mut self, key: HashedKey) {
		self.hand = self.stack
			.before(&key)
			.map(|object| object.key);

		self.stack.remove(&key);
	}

	fn clear(&mut self) {
		self.stack.clear();
		self.hand = None;
	}

	fn evict_one(&mut self) -> Option<HashedKey> {
		loop {
			let key = self.hand.or_else(||
				self.stack
					.back()
					.map(|object| object.key)
			)?;

			self.hand = self.stack
				.before(&key)
				.map(|object| object.key);

			let object = self.stack.get(&key)?;

			if !object.visited {
				self.stack.remove(&key);
				return Some(key);
			}

			self.stack.update(&key, |object| {
				object.visited = false;
			});
		}
	}
}

impl Object {
	fn new(key: HashedKey) -> Self {
		Object {
			key,
			visited: false,
		}
	}
}

impl Borrow<HashedKey> for Object {
	fn borrow(&self) -> &HashedKey {
		&self.key
	}
}

impl Hash for Object {
	fn hash<H>(&self, state: &mut H)
	where
		H: Hasher,
	{
		self.key.hash(state)
	}
}

impl PartialEq for Object {
	fn eq(&self, other: &Self) -> bool {
		self.key == other.key
	}
}

impl Eq for Object {}

#[cfg(test)]
mod tests {
	#[test]
	fn eviction_order_is_correct() {
		use crate::worker::policy::policy_stack::{PolicyStack, SieveStack};

		let mut stack = SieveStack::default();

		for access in [0, 1, 0, 2] {
			stack.insert(access, 1);
		}

		assert_eq!(stack.evict_one(), Some(1));

		for access in [3, 0, 1, 3] {
			stack.insert(access, 1);
		}

		for eviction in [2, 1, 3, 0] {
			assert_eq!(stack.evict_one(), Some(eviction));
		}

		assert_eq!(stack.evict_one(), None);
	}
}
