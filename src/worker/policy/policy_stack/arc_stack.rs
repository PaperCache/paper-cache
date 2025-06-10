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
	CacheSize,
	HashedKey,
	NoHasher,
	policy::PaperPolicy,
	object::ObjectSize,
	worker::policy::policy_stack::PolicyStack,
};

pub struct ArcStack {
	max_size: CacheSize,
	p: f64,

	t1: Stack,
	t2: Stack,

	b1: Stack,
	b2: Stack,
}

#[derive(Default)]
struct Stack {
	stack: HashList<Object, NoHasher>,
	used_size: CacheSize,
}

struct Object {
	key: HashedKey,
	size: ObjectSize,
}

impl PolicyStack for ArcStack {
	fn is_policy(&self, policy: &PaperPolicy) -> bool {
		matches!(policy, PaperPolicy::Arc)
	}

	fn len(&self) -> usize {
		self.t1.stack.len() + self.t2.stack.len()
	}

	fn contains(&self, key: HashedKey) -> bool {
		self.t1.stack.contains(&key) || self.t2.stack.contains(&key)
	}

	fn insert(&mut self, key: HashedKey, size: ObjectSize) {
		if self.t1.stack.contains(&key) || self.t2.stack.contains(&key) {
			// case 1
			self.t1.update(key, size);
			self.t2.update(key, size);

			return self.update(key);
		}

		if let Some(mut object) = self.b1.remove(key) {
			// case 2
			object.size = size;

			let delta = if self.b1.used_size >= self.b2.used_size {
				1.0
			} else {
				self.b2.used_size as f64 / self.b1.used_size as f64
			};

			self.p = if self.p + delta < self.max_size as f64 {
				self.p + delta
			} else {
				self.max_size as f64
			};

			return self.t2.insert(object);
		}

		if let Some(mut object) = self.b2.remove(key) {
			// case 3
			object.size = size;

			let delta = if self.b2.used_size >= self.b1.used_size {
				1.0
			} else {
				self.b1.used_size as f64 / self.b2.used_size as f64
			};

			self.p = if self.p - delta > 0.0 {
				self.p - delta
			} else {
				0.0
			};

			return self.t2.insert(object);
		}

		// case 4 is mostly handled in the `pop` function

		let object = Object::new(key, size);
		self.t1.insert(object);
	}

	fn update(&mut self, key: HashedKey) {
		if let Some(object) = self.t1.remove(key) {
			self.t2.insert(object);
			return;
		}

		if let Some(object) = self.t2.remove(key) {
			self.t2.insert(object);
		}
	}

	fn remove(&mut self, key: HashedKey) {
		self.t1.remove(key);
		self.t2.remove(key);

		self.b1.remove(key);
		self.b2.remove(key);
	}

	fn resize(&mut self, max_size: CacheSize) {
		self.max_size = max_size;
	}

	fn clear(&mut self) {
		self.t1.clear();
		self.t2.clear();

		self.b1.clear();
		self.b2.clear();
	}

	fn evict_one(&mut self) -> Option<HashedKey> {
		if self.t1.used_size + self.b1.used_size >= self.max_size {
			if self.t1.used_size < self.max_size {
				self.b1.pop();
				return self.replace();
			} else {
				return self.t1
					.pop()
					.map(|object| object.key);
			}
		}

		if self.total_used_size() >= self.max_size * 2 {
			self.b2.pop();
		}

		self.replace()
	}
}

impl ArcStack {
	pub fn new(max_size: CacheSize) -> Self {
		ArcStack {
			max_size,
			p: 0.0,

			t1: Stack::default(),
			t2: Stack::default(),

			b1: Stack::default(),
			b2: Stack::default(),
		}
	}

	fn total_used_size(&self) -> CacheSize {
		self.t1.used_size
			+ self.t2.used_size
			+ self.b1.used_size
			+ self.b2.used_size
	}

	fn replace(&mut self) -> Option<HashedKey> {
		if !self.t1.stack.is_empty() && self.t1.used_size as f64 >= self.p || self.t2.stack.is_empty() {
			let object = self.t1.pop()?;
			let key = object.key;

			self.b1.insert(object);
			return Some(key);
		}

		let object = self.t2.pop()?;
		let key = object.key;

		self.b2.insert(object);
		Some(key)
	}
}

impl Stack {
	fn insert(&mut self, object: Object) {
		self.used_size += object.size as CacheSize;
		self.stack.push_front(object);
	}

	fn update(&mut self, key: HashedKey, size: ObjectSize) {
		let Some(object) = self.stack.get(&key) else {
			return;
		};

		self.used_size -= object.size as CacheSize;
		self.used_size += size as CacheSize;

		self.stack.update(&key, |object| object.size = size);
	}

	fn remove(&mut self, key: HashedKey) -> Option<Object> {
		let object = self.stack.remove(&key)?;
		self.used_size -= object.size as CacheSize;

		Some(object)
	}

	fn pop(&mut self) -> Option<Object> {
		let object = self.stack.pop_back()?;
		self.used_size -= object.size as CacheSize;

		Some(object)
	}

	fn clear(&mut self) {
		self.stack.clear();
		self.used_size = 0;
	}
}

impl Object {
	fn new(key: HashedKey, size: ObjectSize) -> Self {
		Object {
			key,
			size,
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
		use crate::worker::policy::policy_stack::{PolicyStack, ArcStack};

		let mut stack = ArcStack::new(4);

		for access in [0, 1, 0, 2, 1, 3, 0, 4, 2, 5, 0] {
			stack.insert(access, 1);
		}

		for eviction in [3, 4, 5, 1, 2, 0] {
			assert_eq!(stack.evict_one(), Some(eviction));
		}

		assert_eq!(stack.evict_one(), None);
	}
}
