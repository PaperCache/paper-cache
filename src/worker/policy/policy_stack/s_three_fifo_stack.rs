use std::{
	cmp,
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

pub struct SThreeFifoStack {
	ratio: f64,

	small: Stack,
	main: Stack,
	ghost: HashList<HashedKey, NoHasher>,
}

struct Stack {
	stack: HashList<Object, NoHasher>,

	used_size: CacheSize,
	max_size: Option<CacheSize>,
}

struct Object {
	key: HashedKey,
	size: ObjectSize,
	freq: u8,
}

impl PolicyStack for SThreeFifoStack {
	fn is_policy(&self, policy: &PaperPolicy) -> bool {
		let PaperPolicy::SThreeFifo(ratio) = policy else {
			return false;
		};

		self.ratio == *ratio
	}

	fn len(&self) -> usize {
		self.small.stack.len() + self.main.stack.len()
	}

	fn contains(&self, key: HashedKey) -> bool {
		self.any_stack_contains(key)
	}

	fn insert(&mut self, key: HashedKey, size: ObjectSize) {
		if self.any_stack_contains(key) {
			self.small.update(key, size);
			self.main.update(key, size);

			return self.update(key);
		}

		let object = Object::new(key, size);

		if self.ghost.contains(&key) {
			self.main.insert(object);
		} else {
			self.small.insert(object);
		}
	}

	fn update(&mut self, key: HashedKey) {
		self.small.stack.update(&key, |object| object.incr_freq());
		self.main.stack.update(&key, |object| object.incr_freq());
	}

	fn remove(&mut self, key: HashedKey) {
		self.small.remove(key);
		self.main.remove(key);
		self.ghost.remove(&key);
	}

	fn resize(&mut self, max_size: CacheSize) {
		self.small.max_size = Some((self.ratio * max_size as f64) as u64);
		self.main.max_size = Some(((1.0 - self.ratio) * max_size as f64) as u64);
	}

	fn clear(&mut self) {
		self.small.clear();
		self.main.clear();
		self.ghost.clear();
	}

	fn pop(&mut self) -> Option<HashedKey> {
		if !self.main.is_full() {
			// prioritize evicting from the small stack when possible
			if let Some(key) = self.evict_small() {
				return Some(key);
			}
		}

		self.evict_main()
	}
}

impl SThreeFifoStack {
	pub fn new(ratio: f64, max_size: CacheSize) -> Self {
		let small = Stack::new(Some((ratio * max_size as f64) as u64));
		let main = Stack::new(Some(((1.0 - ratio) * max_size as f64) as u64));
		let ghost = HashList::with_hasher(NoHasher::default());

		SThreeFifoStack {
			ratio,

			small,
			main,
			ghost,
		}
	}

	fn any_stack_contains(&self, key: HashedKey) -> bool {
		self.small.stack.contains(&key) || self.main.stack.contains(&key)
	}

	fn evict_small(&mut self) -> Option<HashedKey> {
		loop {
			let object = self.small.pop()?;

			if object.freq > 1 {
				self.main.insert(object);

				if self.main.is_full() {
					return self.evict_main();
				}
			} else {
				self.ghost.push_front(object.key);
				return Some(object.key);
			}
		}
	}

	fn evict_main(&mut self) -> Option<HashedKey> {
		loop {
			let mut object = self.main.pop()?;

			if object.freq > 0 {
				object.freq -= 1;
				self.main.insert(object);
			} else {
				while self.ghost.len() > self.main.stack.len() {
					self.ghost.pop_back();
				}

				return Some(object.key);
			}
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

	fn is_full(&self) -> bool {
		let Some(max_stack_size) = self.max_size else {
			return false;
		};

		self.used_size >= max_stack_size
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

			freq: 0,
		}
	}

	fn incr_freq(&mut self) {
		self.freq = cmp::min(self.freq + 1, 3);
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
			worker::policy::policy_stack::{PolicyStack, SThreeFifoStack},
		};

		let accesses: Vec<HashedKey> = vec![0, 1, 0, 2, 1, 3, 0, 4, 2, 5, 0];
		let mut evictions: Vec<HashedKey> = vec![0, 5, 4, 3, 2, 1];

		let mut stack = SThreeFifoStack::new(0.5, 4);

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
