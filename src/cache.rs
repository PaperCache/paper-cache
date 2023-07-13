use std::fmt::Display;
use std::hash::Hash;
use rustc_hash::FxHashMap;
use kwik::utils;
use crate::cache_error::{CacheError, ErrorKind};
use crate::stats::Stats;
use crate::object::{Object, MemSize};
use crate::policy::Policy;
use crate::expiries::Expiries;

use crate::policy_stack::{
	PolicyStack,
	LfuStack,
	FifoStack,
	LruStack,
	MruStack,
};

pub type CacheSize = u64;

pub struct Cache<K, V>
where
	K: 'static + Eq + Hash + Clone + Display + Sync,
	V: 'static + Clone + Sync + MemSize,
{
	stats: Stats,

	policies: Vec<&'static Policy>,
	policy_stacks: Vec<Box<dyn PolicyStack<K>>>,

	expiries: Expiries<K>,

	objects: FxHashMap<K, Object<V>>,
}

impl<K, V> Cache<K, V>
where
	K: 'static + Eq + Hash + Clone + Display + Sync,
	V: 'static + Clone + Sync + MemSize,
{
	pub fn new(
		max_size: CacheSize,
		policies: Option<Vec<&'static Policy>>
	) -> Result<Self, CacheError> {
		if max_size == 0 {
			return Err(CacheError::new(
				ErrorKind::InvalidCacheSize,
				"The cache size cannot be zero."
			));
		}

		let policies = match policies {
			Some(policies) => {
				if policies.is_empty() {
					return Err(CacheError::new(
						ErrorKind::InvalidPolicies,
						"Invalid policies."
					));
				}

				policies
			},

			None => vec![
				&Policy::Lfu,
				&Policy::Fifo,
				&Policy::Lru,
				&Policy::Mru,
			],
		};

		let policy_stacks: Vec::<Box<dyn PolicyStack<K>>> = vec![
			Box::new(LfuStack::<K>::new()),
			Box::new(FifoStack::<K>::new()),
			Box::new(LruStack::<K>::new()),
			Box::new(MruStack::<K>::new()),
		];

		let cache = Cache {
			stats: Stats::new(max_size, policies[0].to_owned()),

			policies,
			policy_stacks,

			expiries: Expiries::new(),

			objects: FxHashMap::default(),
		};

		Ok(cache)
	}

	pub fn stats(&self) -> Stats {
		self.stats
	}

	pub fn get(&mut self, key: &K) -> Result<V, CacheError> {
		match self.objects.get_key_value(key) {
			Some((key, object)) => {
				self.stats.hit();

				for policy in &self.policies {
					self.policy_stacks[policy.index()].update(key);
				}

				Ok(object.get_data().clone())
			},

			None => {
				self.stats.miss();

				Err(CacheError::new(
					ErrorKind::KeyNotFound,
					"The key was not found in the cache."
				))
			},
		}
	}

	pub fn set(&mut self, key: K, value: V, ttl: Option<u32>) -> Result<(), CacheError> {
		let object = Object::new(value, ttl);
		let size = object.get_size();

		if size == 0 {
			return Err(CacheError::new(
				ErrorKind::InvalidValueSize,
				"The value size cannot be zero."
			));
		}

		if self.stats.exceeds_max_size(size) {
			return Err(CacheError::new(
				ErrorKind::InvalidValueSize,
				"The value size cannot be larger than the cache size."
			));
		}

		self.stats.set();

		self.reduce(self.stats.target_used_size_to_fit(size))?;

		for policy in &self.policies {
			self.policy_stacks[policy.index()].insert(&key);
		}

		self.expiries.insert(&key, object.get_expiry());

		if let Some(old_object) = self.objects.insert(key, object) {
			self.stats.decrease_used_size(old_object.get_size());
		}

		self.stats.increase_used_size(size);

		Ok(())
	}

	pub fn del(&mut self, key: &K) -> Result<(), CacheError> {
		match self.objects.remove(key) {
			Some(object) => {
				self.stats.del();
				self.stats.decrease_used_size(object.get_size());

				for policy in &self.policies {
					self.policy_stacks[policy.index()].remove(key);
				}

				self.expiries.remove(key, object.get_expiry());

				Ok(())
			},

			None => Err(CacheError::new(
				ErrorKind::KeyNotFound,
				"The key was not found in the cache."
			)),
		}
	}

	pub fn wipe(&mut self) -> Result<(), CacheError> {
		self.objects.clear();
		self.expiries.clear();

		for policy in &self.policies {
			self.policy_stacks[policy.index()].clear();
		}

		self.stats.reset_used_size();

		Ok(())
	}

	pub fn resize(&mut self, max_size: CacheSize) -> Result<(), CacheError> {
		if max_size == 0 {
			return Err(CacheError::new(
				ErrorKind::InvalidCacheSize,
				"The cache size cannot be zero."
			));
		}

		self.reduce(max_size)?;
		self.stats.set_max_size(max_size);

		Ok(())
	}

	pub fn policy(&mut self, policy: &'static Policy) -> Result<(), CacheError> {
		if !self.policies.contains(&policy) {
			return Err(CacheError::new(
				ErrorKind::InvalidPolicy,
				"The supplied policy is not one of the cache's considered policies."
			));
		}

		self.stats.set_policy(policy.to_owned());

		Ok(())
	}

	/// Reduces the cache size to the maximum size.
	fn reduce(&mut self, target_size: CacheSize) -> Result<(), CacheError> {
		while self.stats.used_size_exceeds(target_size) {
			let policy_key = self.policy_stacks[
				self.stats.get_policy().index()
			].get_eviction();

			if let Some(key) = &policy_key {
				if self.del(key).is_err() {
					return Err(CacheError::new(
						ErrorKind::Internal,
						"An internal error has occured."
					));
				}
			} else {
				return Err(CacheError::new(
					ErrorKind::Internal,
					"An internal error has occured."
				));
			}
		}

		Ok(())
	}

	/// Removes any expired objects from the cache.
	pub fn prune_expired(&mut self) {
		let now = utils::timestamp();

		while let Some(expired) = self.expiries.expired(now) {
			for key in expired {
				let _ = self.del(&key);
			}
		}
	}
}

unsafe impl<K, V> Send for Cache<K, V>
where
	K: 'static + Eq + Hash + Clone + Display + Sync,
	V: 'static + Clone + Sync + MemSize,
{}
