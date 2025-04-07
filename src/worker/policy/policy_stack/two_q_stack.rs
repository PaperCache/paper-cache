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

pub struct TwoQStack {
	k_in: f64,
	k_out: f64,

	a1_in: Stack,
	a1_out: Stack,
	am: Stack,
}

struct Stack {
	stack: HashList<Object, NoHasher>,

	used_size: CacheSize,
	max_size: Option<CacheSize>,
}

struct Object {
	key: HashedKey,
	size: ObjectSize,
}

impl PolicyStack for TwoQStack {
	fn is_policy(&self, policy: &PaperPolicy) -> bool {
		let PaperPolicy::TwoQ(k_in, k_out) = policy else {
			return false;
		};

		self.k_in == *k_in && self.k_out == *k_out
	}

	fn len(&self) -> usize {
		self.a1_in.stack.len()
			+ self.a1_out.stack.len()
			+ self.am.stack.len()
	}

	fn insert(&mut self, key: HashedKey, size: ObjectSize) {
		if self.contains(key) {
			self.a1_in.update(key, size);
			self.a1_out.update(key, size);
			self.am.update(key, size);

			return self.update(key);
		}

		self.restructure_to_fit(size);

		let object = Object::new(key, size);
		self.a1_in.insert(object);
	}

	fn update(&mut self, key: HashedKey) {
		if let Some(object) = self.a1_out.remove(key) {
			return self.am.insert(object);
		}

		self.am.stack.move_front(&key);
	}

	fn remove(&mut self, key: HashedKey) {
		self.a1_in.remove(key);
		self.a1_out.remove(key);
		self.am.remove(key);
	}

	fn resize(&mut self, max_size: CacheSize) {
		self.a1_in.max_size = Some((self.k_in * max_size as f64) as u64);
		self.a1_out.max_size = Some((self.k_out * max_size as f64) as u64);
	}

	fn clear(&mut self) {
		self.a1_in.clear();
		self.a1_out.clear();
		self.am.clear();
	}

	fn pop(&mut self) -> Option<HashedKey> {
		if let Some(object) = self.a1_out.pop() {
			return Some(object.key);
		}

		if let Some(object) = self.a1_in.pop() {
			return Some(object.key);
		}

		self.am
			.pop()
			.map(|object| object.key)
	}
}

impl TwoQStack {
	pub fn new(k_in: f64, k_out: f64, max_size: CacheSize) -> Self {
		let a1_in = Stack::new(Some((k_in * max_size as f64) as u64));
		let a1_out = Stack::new(Some((k_out * max_size as f64) as u64));
		let am = Stack::new(None);

		TwoQStack {
			k_in,
			k_out,

			a1_in,
			a1_out,
			am,
		}
	}

	fn contains(&self, key: HashedKey) -> bool {
		self.a1_in.stack.contains(&key)
			|| self.a1_out.stack.contains(&key)
			|| self.am.stack.contains(&key)
	}

	fn restructure_to_fit(&mut self, object_size: ObjectSize) {
		while !self.a1_in.can_fit(object_size) {
			let Some(object) = self.a1_in.pop() else {
				return;
			};

			self.a1_out.insert(object);
		}
	}
}

impl Stack {
	fn new(max_size: Option<CacheSize>) -> Self {
		Stack {
			stack: HashList::with_hasher(NoHasher::default()),

			used_size: 0,
			max_size,
		}
	}

	fn can_fit(&self, object_size: ObjectSize) -> bool {
		let Some(max_stack_size) = self.max_size else {
			return true;
		};

		self.used_size + object_size as CacheSize <= max_stack_size
	}

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
		use crate::{
			HashedKey,
			worker::policy::policy_stack::{PolicyStack, TwoQStack},
		};

		let accesses: Vec<HashedKey> = vec![0, 1, 0, 2, 1, 3, 0, 4, 2, 5, 0];
		let mut evictions: Vec<HashedKey> = vec![0, 2, 1, 5, 4, 3];

		let mut stack = TwoQStack::new(0.25, 0.5, 4);

		for access in accesses {
			stack.insert(access, 1);
		}

		let mut eviction_count = 0;

		while let Some(key) = stack.pop() {
			match evictions.pop() {
				Some(eviction) => assert_eq!(key, eviction),
				None => unreachable!(),
			}

			eviction_count += 1;
		}

		assert_eq!(eviction_count, 6);
	}
}
