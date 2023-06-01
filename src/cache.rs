use std::fmt::Display;
use std::hash::Hash;
use std::collections::BTreeMap;
use rustc_hash::FxHashMap;
use kwik::utils;
use crate::cache_error::{CacheError, ErrorKind};
use crate::stats::Stats;
use crate::object::{Object, MemSize};
use crate::policy::Policy;
use crate::policy_stack::{PolicyStack, LruStack, MruStack};

pub type CacheSize = u64;

pub struct Cache<K, V>
where
	K: 'static + Eq + Hash + Copy + Display + Sync,
	V: 'static + Clone + Sync + MemSize,
{
	stats: Stats,

	policies: Vec<&'static Policy>,
	policy: &'static Policy,
	policy_stacks: Vec<Box<dyn PolicyStack<K>>>,

	expiries: BTreeMap<u64, K>,

	objects: FxHashMap<K, Object<V>>,
}

impl<K, V> Cache<K, V>
where
	K: 'static + Eq + Hash + Copy + Display + Sync,
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

			None => vec![&Policy::Lru, &Policy::Mru],
		};

		let policy_stacks: Vec::<Box<dyn PolicyStack<K>>> = vec![
			Box::new(LruStack::<K>::new()),
			Box::new(MruStack::<K>::new()),
		];

		let initial_policy = policies[0];

		let cache = Cache {
			stats: Stats::new(max_size),

			policies,
			policy: initial_policy,
			policy_stacks,

			expiries: BTreeMap::new(),

			objects: FxHashMap::default(),
		};

		Ok(cache)
	}

	pub fn stats(&self) -> Stats {
		self.stats
	}

	pub fn get(&mut self, key: &K) -> Result<V, CacheError> {
		match self.objects.get_mut(key) {
			Some(object) => {
				self.stats.hit();

				for policy in &self.policies {
					self.policy_stacks[policy.index()].update(&key);
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
		let expiry = object.get_expiry();

		if size == 0 {
			return Err(CacheError::new(
				ErrorKind::InvalidValueSize,
				"The value size cannot be zero."
			));
		}

		if !self.stats.max_size_exceeds(&size) {
			return Err(CacheError::new(
				ErrorKind::InvalidValueSize,
				"The value size cannot be larger than the cache size."
			));
		}

		self.reduce(&self.stats.target_used_size_to_fit(&size))?;

		self.objects.insert(key, object);
		self.stats.increase_used_size(&size);

		for policy in &self.policies {
			self.policy_stacks[policy.index()].insert(&key);
		}

		if let Some(expiry) = expiry {
			self.expiries.insert(expiry, key);
		}

		Ok(())
	}

	pub fn del(&mut self, key: &K) -> Result<(), CacheError> {
		match self.objects.remove(key) {
			Some(object) => {
				self.stats.decrease_used_size(&object.get_size());

				for policy in &self.policies {
					self.policy_stacks[policy.index()].remove(key);
				}

				Ok(())
			},

			None => Err(CacheError::new(
				ErrorKind::KeyNotFound,
				"The key was not found in the cache."
			)),
		}
	}

	pub fn clear(&mut self) -> Result<(), CacheError> {
		self.objects.clear();
		self.expiries.clear();

		for policy in &self.policies {
			self.policy_stacks[policy.index()].clear();
		}

		self.stats.reset_used_size();

		Ok(())
	}

	pub fn resize(&mut self, max_size: &CacheSize) -> Result<(), CacheError> {
		if *max_size == 0 {
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

		self.policy = policy;
		Ok(())
	}

	/// Reduces the cache size to the maximum size.
	fn reduce(&mut self, target_size: &CacheSize) -> Result<(), CacheError> {
		while self.stats.used_size_exceeds(target_size) {
			let policy_key = self.policy_stacks[self.policy.index()].get_eviction();

			if let Some(key) = &policy_key {
				if let Err(_) = self.del(key) {
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

		while let Some((&expiry, &key)) = self.expiries.iter().next() {
			if expiry > now {
				return;
			}

			let _ = self.del(&key);
			self.expiries.remove(&expiry);
		}
	}
}

unsafe impl<K, V> Send for Cache<K, V>
where
	K: 'static + Eq + Hash + Copy + Display + Sync,
	V: 'static + Clone + Sync + MemSize,
{}
