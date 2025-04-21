use std::collections::HashMap;

use crate::{
	CacheSize,
	HashedKey,
	NoHasher,
	policy::PaperPolicy,
	object::ObjectSize,
	worker::policy::policy_stack::{PolicyStack, init_policy_stack},
};

pub struct MiniStack {
	stack: Box<dyn PolicyStack>,
	sizes: HashMap<HashedKey, ObjectSize, NoHasher>,

	policy: PaperPolicy,

	max_size: CacheSize,
	used_size: CacheSize,

	count: u64,
	hits: u64,
}

impl MiniStack {
	pub fn new(policy: PaperPolicy, size: CacheSize) -> Self {
		MiniStack {
			stack: init_policy_stack(policy, size),
			sizes: HashMap::with_hasher(NoHasher::default()),

			policy,

			max_size: size,
			used_size: 0,

			count: 0,
			hits: 0,
		}
	}

	pub fn policy(&self) -> PaperPolicy {
		self.policy
	}

	pub fn miss_ratio(&self) -> f64 {
		match self.count {
			0 => 1.0,
			count => 1.0 - self.hits as f64 / count as f64,
		}
	}

	pub fn update_with_count(&mut self, key: HashedKey) {
		self.count += 1;

		if self.stack.contains(key) {
			self.hits += 1;
		}

		self.update(key);
	}

	fn reduce(&mut self, target_size: CacheSize) {
		while self.used_size > target_size {
			let maybe_object_size = self.stack
				.pop()
				.and_then(|evict_key| self.sizes.remove(&evict_key));

			if let Some(object_size) = maybe_object_size {
				self.used_size -= object_size as CacheSize;
			};
		}
	}
}

impl PolicyStack for MiniStack {
	fn is_policy(&self, policy: &PaperPolicy) -> bool {
		self.stack.is_policy(policy)
	}

	fn len(&self) -> usize {
		self.stack.len()
	}

	fn contains(&self, key: HashedKey) -> bool {
		self.stack.contains(key)
	}

	fn insert(&mut self, key: HashedKey, size: ObjectSize) {
		self.reduce(self.max_size - size as CacheSize);

		if let Some(old_size) = self.sizes.insert(key, size) {
			self.used_size -= old_size as CacheSize;
		}

		self.used_size += size as CacheSize;
		self.stack.insert(key, size);
	}

	fn update(&mut self, key: HashedKey) {
		self.stack.update(key);
	}

	fn remove(&mut self, key: HashedKey) {
		self.stack.remove(key);

		if let Some(size) = self.sizes.remove(&key) {
			self.used_size -= size as CacheSize;
		}
	}

	fn resize(&mut self, size: CacheSize) {
		self.stack.resize(size);

		self.max_size = size;
		self.reduce(self.max_size);
	}

	fn clear(&mut self) {
		self.stack.clear();

		self.sizes.clear();
		self.used_size = 0;

		self.count = 0;
		self.hits = 0;
	}

	fn pop(&mut self) -> Option<HashedKey> {
		let maybe_key = self.stack.pop();
		let maybe_size = maybe_key.and_then(|key| self.sizes.remove(&key));

		if let Some(size) = maybe_size {
			self.used_size -= size as CacheSize;
		}

		maybe_key
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn used_size_after_reinsert_is_correct() {
		use crate::{
			PaperPolicy,
			worker::policy::{
				policy_stack::PolicyStack,
				mini_stack::MiniStack,
			},
		};

		let mut mini_stack = MiniStack::new(PaperPolicy::Lfu, 100);
		assert_eq!(mini_stack.used_size, 0);

		mini_stack.insert(0, 5);
		assert_eq!(mini_stack.used_size, 5);

		mini_stack.insert(1, 2);
		assert_eq!(mini_stack.used_size, 7);

		mini_stack.insert(0, 4);
		assert_eq!(mini_stack.used_size, 6);

		mini_stack.insert(0, 6);
		assert_eq!(mini_stack.used_size, 8);
	}

	#[test]
	fn used_size_after_remove_is_correct() {
		use crate::{
			PaperPolicy,
			worker::policy::{
				policy_stack::PolicyStack,
				mini_stack::MiniStack,
			},
		};

		let mut mini_stack = MiniStack::new(PaperPolicy::Lfu, 100);
		assert_eq!(mini_stack.used_size, 0);

		mini_stack.insert(0, 5);
		assert_eq!(mini_stack.used_size, 5);

		mini_stack.insert(1, 2);
		assert_eq!(mini_stack.used_size, 7);

		mini_stack.remove(0);
		assert_eq!(mini_stack.used_size, 2);
	}
}
