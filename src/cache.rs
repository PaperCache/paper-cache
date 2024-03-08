/*use std::{
	sync::atomic::AtomicU64,
	rc::Rc,
	hash::Hash,
	collections::HashMap,
};

use thiserror::Error;
use kwik::utils;

use crate::{
	stats::Stats,
	object::{Object, MemSize},
	policy::{
		Policy,
		PolicyType,
		PolicyStack,
	},
	expiries::Expiries,
};

pub type CacheSize = u64;

pub struct Cache<K, V>
where
	K: 'static + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{
	stats: Stats,

	policy_stacks: Vec<PolicyType<K>>,
	expiries: Expiries<K>,

	objects: HashMap<Rc<K>, Object<V>>,
}

#[derive(Debug, Error)]
pub enum CacheError {
	#[error("The policies cannot be empty.")]
	EmptyPolicies,

	#[error("The supplied policy is not one of the cache's configured policies.")]
	UnconfiguredPolicy,

	#[error("The key was not found in the cache.")]
	KeyNotFound,

	#[error("The value size cannot be zero.")]
	ZeroValueSize,

	#[error("The value size cannot exceed the cache size.")]
	ExceedingValueSize,

	#[error("The cache size cannot be zero.")]
	ZeroCacheSize,

	#[error("Internal error.")]
	Internal,
}

impl<K, V> Cache<K, V>
where
	K: 'static + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{
	pub fn new(
		max_size: CacheSize,
		policies: Option<Vec<Policy>>
	) -> Result<Self, CacheError> {
		if max_size == 0 {
			return Err(CacheError::ZeroCacheSize);
		}

		let policies = match policies {
			Some(policies) => {
				if policies.is_empty() {
					return Err(CacheError::EmptyPolicies);
				}

				policies
			},

			None => vec![
				Policy::Lfu,
				Policy::Fifo,
				Policy::Lru,
				Policy::Mru,
			],
		};

		let policy_stacks: Vec<PolicyType<K>> = policies
			.iter()
			.map(|policy| policy.as_policy_type())
			.collect::<Vec<PolicyType<K>>>();

		let cache = Cache {
			stats: Stats::new(max_size, policies[0]),

			policy_stacks,
			expiries: Expiries::new(),

			objects: HashMap::new(),
		};

		Ok(cache)
	}

	#[must_use]
	pub fn stats(&self) -> Stats {
		self.stats
	}

	pub fn get(&mut self, key: &K) -> Result<Rc<V>, CacheError> {
		match self.objects.get_key_value(key) {
			Some((key, object)) => {
				self.stats.hit();

				for policy_stack in self.policy_stacks.iter_mut() {
					policy_stack.update(key);
				}

				Ok(Rc::clone(object.get_data()))
			},

			None => {
				self.stats.miss();
				Err(CacheError::KeyNotFound)
			},
		}
	}

	pub fn set(&mut self, key: K, value: V, ttl: Option<u32>) -> Result<(), CacheError> {
		let key = Rc::new(key);
		let object = Object::new(value, ttl);
		let size = object.get_size();

		if size == 0 {
			return Err(CacheError::ZeroValueSize);
		}

		if self.stats.exceeds_max_size(size) {
			return Err(CacheError::ExceedingValueSize);
		}

		self.stats.set();

		self.reduce(self.stats.target_used_size_to_fit(size))?;

		for policy_stack in self.policy_stacks.iter_mut() {
			policy_stack.insert(&key);
		}

		self.expiries.insert(&key, object.get_expiry());

		if let Some(old_object) = self.objects.insert(key, object) {
			self.stats.decrease_used_size(old_object.get_size());
		}

		self.stats.increase_used_size(size);

		Ok(())
	}

	pub fn del(&mut self, key: &K) -> Result<(), CacheError> {
		match self.erase(key) {
			Some(_) => {
				self.stats.del();
				Ok(())
			},

			None => Err(CacheError::KeyNotFound),
		}
	}

	pub fn has(&self, key: &K) -> bool {
		self.objects.contains_key(key)
	}

	pub fn peek(&self, key: &K) -> Result<Rc<V>, CacheError> {
		self.objects.get(key)
			.map(|object| object.get_data()).cloned()
			.ok_or(CacheError::KeyNotFound)
	}

	pub fn wipe(&mut self) -> Result<(), CacheError> {
		self.objects.clear();
		self.expiries.clear();

		for policy_stack in self.policy_stacks.iter_mut() {
			policy_stack.clear();
		}

		self.stats.reset_used_size();

		Ok(())
	}

	pub fn resize(&mut self, max_size: CacheSize) -> Result<(), CacheError> {
		if max_size == 0 {
			return Err(CacheError::ZeroCacheSize);
		}

		self.reduce(max_size)?;
		self.stats.set_max_size(max_size);

		Ok(())
	}

	pub fn policy(&mut self, policy: Policy) -> Result<(), CacheError> {
		if !self.policy_stacks.iter().any(|policy_stack| policy == policy_stack) {
			return Err(CacheError::UnconfiguredPolicy);
		}

		self.stats.set_policy(policy);

		Ok(())
	}

	/// Reduces the cache size to the maximum size.
	fn reduce(&mut self, target_size: CacheSize) -> Result<(), CacheError> {
		let policy_index = self.policy_stacks
			.iter()
			.position(|policy_stack| self.stats.get_policy() == policy_stack);

		let Some(policy_index) = policy_index else {
			return Err(CacheError::Internal);
		};

		while self.stats.used_size_exceeds(target_size) {
			let policy_key = self.policy_stacks[policy_index].get_eviction();

			if let Some(key) = &policy_key {
				if self.erase(key).is_none() {
					return Err(CacheError::Internal);
				}
			} else {
				return Err(CacheError::Internal);
			}
		}

		Ok(())
	}

	/// Removes any expired objects from the cache.
	pub fn prune_expired(&mut self) {
		let now = utils::timestamp();

		while let Some(expired) = self.expiries.expired(now) {
			for key in expired {
				let _ = self.erase(&key);
			}
		}
	}

	fn erase(&mut self, key: &K) -> Option<Object<V>> {
		let object = self.objects.remove(key)?;

		self.stats.decrease_used_size(object.get_size());

		for policy_stack in self.policy_stacks.iter_mut() {
			policy_stack.remove(key);
		}

		self.expiries.remove(key, object.get_expiry());

		Some(object)
	}
}

unsafe impl<K, V> Send for Cache<K, V>
where
	K: 'static + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{}*/
