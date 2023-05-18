use std::hash::Hash;
use std::collections::BTreeMap;
use rustc_hash::FxHashMap;
use kwik::utils;
use crate::cache_error::{CacheError, ErrorKind};
use crate::stats::Stats;
use crate::object::Object;
use crate::policy::Policy;
use crate::policy_stack::{PolicyStack, LruStack, MruStack};

pub type CacheSize = u64;
pub type SizeOfObject<V> = fn(&V) -> u64;

pub struct PaperCache<K, V>
where
	K: Eq + Hash + Copy + 'static + std::fmt::Display,
{
	stats: Stats,

	policies: Vec<Policy>,
	policy: Policy,
	policy_stacks: Vec<Box<dyn PolicyStack<K>>>,

	expiries: BTreeMap<u64, K>,

	objects: FxHashMap<K, Object<V>>,
	size_of_object: SizeOfObject<V>,
}

impl<K, V> PaperCache<K, V>
where
	K: Eq + Hash + Copy + 'static + std::fmt::Display,
{
	/// Creates an empty PaperCache with maximum size `max_size`.
	/// If the maximum size is zero, a [`CacheError`] will be returned.
	/// The cache will only consider eviction policies specified
	/// by `policies` and return an error if the number of supplied
	/// `policies` is zero. If `None` is passed here, the cache
	/// will consider all eviction policies.
	///
	/// The cache's initial eviction policy will be LRU.
	///
	/// # Examples
	///
	/// ```
	/// use paper_cache::{PaperCache, Policy};
	///
	/// assert_eq!(PaperCache::<u32, u32>::new(100, Some(&[&Policy::Lru])), Ok(_));
	///
	/// // Supplying a maximum size of zero will return a CacheError.
	/// assert_eq!(PaperCache::<u32, u32>::new(0, Some(&[&Policy::Lru])), Err(_));
	///
	/// // Supplying an empty policies slice will return a CacheError.
	/// assert_eq!(PaperCache::<u32, u32>::new(0, Some(&[])), Err(_));
	/// ```
	pub fn new(
		max_size: CacheSize,
		size_of_object: SizeOfObject<V>,
		policies: Option<Vec<Policy>>
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

			None => vec![Policy::Lru, Policy::Mru],
		};

		let policy_stacks: Vec::<Box<dyn PolicyStack<K>>> = vec![
			Box::new(LruStack::<K>::new()),
			Box::new(MruStack::<K>::new()),
		];

		let paper_cache = PaperCache {
			stats: Stats::new(max_size),

			policies,
			policy: Policy::Lru,
			policy_stacks,

			expiries: BTreeMap::new(),

			objects: FxHashMap::default(),
			size_of_object,
		};

		Ok(paper_cache)
	}

	/// Returns the current statistics.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(100, None);
	///
	/// cache.set(0, 1, None);
	///
	/// assert_eq!(cache.stats().get_used_size(), 1);
	/// ```
	pub fn stats(&self) -> &Stats {
		&self.stats
	}

	/// Gets the value associated with the supplied key.
	/// If the key was not found in the cache, returns a
	/// [`CacheError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(100, None);
	///
	/// cache.set(0, 1, None);
	///
	/// // Getting a key which exists in the cache will return the associated value.
	/// assert_eq!(cache.get(0), Ok(1));
	///	// Getting a key which does not exist in the cache will return a CacheError.
	/// assert_eq!(cache.get(1), Err(_));
	/// ```
	pub fn get(&mut self, key: &K) -> Result<&V, CacheError> {
		match self.objects.get_mut(key) {
			Some(object) => {
				if object.is_expired() {
					self.stats.miss();

					return Err(CacheError::new(
						ErrorKind::KeyNotFound,
						"The key was not found in the cache."
					));
				}

				self.stats.hit();

				for policy in &self.policies {
					let index = get_policy_index(policy);
					self.policy_stacks[index].update(&key);
				}

				Ok(object.get_data())
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

	/// Sets the supplied key and value in the cache.
	/// Returns a [`CacheError`] if the value size is zero or larger than
	/// the cache's maximum size.
	///
	/// If the key already exists in the cache, the associated value is updated
	/// to the supplied value.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(100, None);
	///
	/// assert_eq!(cache.set(0, 1, None), Ok(_));
	/// ```
	pub fn set(&mut self, key: K, value: V, ttl: Option<u32>) -> Result<(), CacheError> {
		self.prune_expired();

		let object = Object::new(value, ttl);
		let size = object.get_size(&self.size_of_object);
		let expiry = *object.get_expiry();

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
			let index = get_policy_index(policy);
			self.policy_stacks[index].insert(&key);
		}

		if let Some(_) = ttl {
			self.expiries.insert(expiry, key);
		}

		Ok(())
	}

	/// Deletes the object associated with the supplied key in the cache.
	/// Returns a [`CacheError`] if the key was not found in the cache.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(100, None);
	///
	/// cache.set(0, 1, None);
	/// assert_eq!(cache.del(0), Ok(_));
	///
	/// // Deleting a key which does not exist in the cache will return a CacheError.
	/// assert_eq!(cache.del(1), Err(_));
	/// ```
	pub fn del(&mut self, key: &K) -> Result<(), CacheError> {
		match self.objects.remove(key) {
			Some(object) => {
				self.stats.decrease_used_size(&object.get_size(&self.size_of_object));

				for policy in &self.policies {
					let index = get_policy_index(policy);
					self.policy_stacks[index].remove(key);
				}

				if object.is_expired() {
					return Err(CacheError::new(
						ErrorKind::KeyNotFound,
						"The key was not found in the cache."
					));
				}

				Ok(())
			},

			None => Err(CacheError::new(
				ErrorKind::KeyNotFound,
				"The key was not found in the cache."
			)),
		}
	}

	/// Resizes the cache to the supplied maximum size.
	/// If the supplied size is zero, returns a [`CacheError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(100, None);
	///
	/// assert_eq!(cache.resize(&1), Ok(_));
	///
	/// // Resizing to a size of zero will return a CacheError.
	/// assert_eq!(cache.resize(&0), Err(_));
	/// ```
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

	/// Sets the eviction policy of the cache to the supplied policy.
	/// If the supplied policy is not one of the considered eviction policies,
	/// a [`CacheError`] is returned.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, Policy};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(100, Some(&[Policy::Lru]));
	///
	/// assert_eq!(cache.policy(&Policy::Lru), Ok(_));
	///
	/// // Supplying a policy that is not one of the considered policies will return a CacheError.
	/// assert_eq!(cache.policy(&Policy::Mru), Err(_));
	/// ```
	pub fn policy(&mut self, policy: Policy) -> Result<(), CacheError> {
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
			let policy_index = get_policy_index(&self.policy);
			let policy_key = self.policy_stacks[policy_index].get_eviction();

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
	fn prune_expired(&mut self) {
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

fn get_policy_index(policy: &Policy) -> usize {
	match policy {
		Policy::Lru => 0,
		Policy::Mru => 1,
	}
}

unsafe impl<K, V> Send for PaperCache<K, V>
where
	K: Eq + Hash + Copy + 'static + std::fmt::Display,
{}
