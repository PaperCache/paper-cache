use std::hash::Hash;
use rustc_hash::FxHashMap;
use crate::error::{PaperError, ErrorKind};
use crate::object::Object;
use crate::policy::Policy;
use crate::policy_stack::{PolicyStack, LruStack};

pub type CacheSize = usize;

pub struct PaperCache<'a, K, V>
where
	K: Eq + Hash + Copy + 'static + std::fmt::Display,
{
	max_size: CacheSize,
	current_size: CacheSize,

	policies: &'a [&'a Policy],
	policy: &'a Policy,

	objects: FxHashMap<K, Object<V>>,

	policy_stacks: Vec<Box<dyn PolicyStack<K>>>,
}

impl<'a, K, V> PaperCache<'a, K, V>
where
	K: Eq + Hash + Copy + 'static + std::fmt::Display,
{
	/// Creates an empty PaperCache with maximum size `max_size`.
	/// If the maximum size is zero, a [`PaperError`] will be returned.
	/// The cache will only consider eviction policies specified
	/// by `policies` and return an error if the number of supplied
	/// `policies` is zero. If `None` is passed here, the cache
	/// will consider all eviction policies.
	///
	/// # Examples
	///
	/// ```
	/// use paper_cache::{PaperCache, Policy};
	///
	/// assert_eq!(PaperCache::<u32, u32>::new(100, Some(&[&Policy::Lru])), Ok(_));
	///
	/// // Supplying a maximum size of zero will return a PaperError.
	/// assert_eq!(PaperCache::<u32, u32>::new(0, Some(&[&Policy::Lru])), Err(_));
	///
	/// // Supplying an empty policies slice will return a PaperError.
	/// assert_eq!(PaperCache::<u32, u32>::new(0, Some(&[])), Err(_));
	/// ```
	pub fn new(
		max_size: CacheSize,
		policies: Option<&'a [&'a Policy]>
	) -> Result<Self, PaperError> {
		if max_size == 0 {
			return Err(PaperError::new(
				ErrorKind::InvalidCacheSize,
				"The cache size cannot be zero."
			));
		}

		let policies = match policies {
			Some(policies) => {
				if policies.is_empty() {
					return Err(PaperError::new(
						ErrorKind::InvalidPolicies,
						"Invalid policies."
					));
				}

				policies
			},
			None => &[&Policy::Lru],
		};

		let policy_stacks: Vec::<Box<dyn PolicyStack<K>>> = vec![
			Box::new(LruStack::<K>::new()),
		];

		let paper_cache = PaperCache {
			max_size,
			current_size: 0,

			policies,
			policy: policies[0],

			objects: FxHashMap::default(),

			policy_stacks,
		};

		Ok(paper_cache)
	}

	/// Gets the value associated with the supplied key.
	/// If the key was not found in the cache, returns a
	/// [`PaperError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(100, None);
	///
	/// cache.set(0, 1);
	///
	/// // Getting a key which exists in the cache will return the associated value.
	/// assert_eq!(cache.get(0), Ok(1));
	///	// Getting a key which does not exist in the cache will return a PaperError.
	/// assert_eq!(cache.get(1), Err(_));
	///
	/// ```
	pub fn get(&mut self, key: &K) -> Result<&V, PaperError> {
		match self.objects.get_mut(key) {
			Some(object) => {
				for policy in self.policies {
					let index = get_policy_index(policy);
					self.policy_stacks[index].update(&key);
				}

				Ok(object.get_data())
			},

			None => Err(PaperError::new(
				ErrorKind::KeyNotFound,
				"The key was not found in the cache."
			)),
		}
	}

	/// Sets the supplied key and value in the cache.
	/// Returns a [`PaperError`] if the value size is zero or larger than
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
	/// assert_eq!(cache.set(0, 1), Ok(_));
	/// ```
	pub fn set(&mut self, key: K, value: V) -> Result<(), PaperError> {
		let object = Object::new(value);
		let size = object.get_size();

		if size == 0 {
			return Err(PaperError::new(
				ErrorKind::InvalidValueSize,
				"The value size cannot be zero."
			));
		}

		if size > self.max_size {
			return Err(PaperError::new(
				ErrorKind::InvalidValueSize,
				"The value size cannot be larger than the cache size."
			));
		}

		self.reduce(&(self.max_size - size))?;

		self.objects.insert(key, object);
		self.current_size += size;

		for policy in self.policies {
			let index = get_policy_index(policy);
			self.policy_stacks[index].insert(&key);
		}

		Ok(())
	}

	/// Deletes the object associated with the supplied key in the cache.
	/// Returns a [`PaperError`] if the key was not found in the cache.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(100, None);
	///
	/// cache.set(0, 1);
	/// assert_eq!(cache.del(0), Ok(_));
	///
	/// // Deleting a key which does not exist in the cache will return a PaperError.
	/// assert_eq!(cache.del(1), Err(_));
	/// ```
	pub fn del(&mut self, key: &K) -> Result<(), PaperError> {
		match self.objects.remove(key) {
			Some(object) => {
				self.current_size -= object.get_size();

				for policy in self.policies {
					let index = get_policy_index(policy);
					self.policy_stacks[index].remove(key);
				}

				Ok(())
			},

			None => Err(PaperError::new(
				ErrorKind::KeyNotFound,
				"The key was not found in the cache."
			)),
		}
	}

	/// Resizes the cache to the supplied maximum size.
	/// If the supplied size is zero, returns a [`PaperError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(100, None);
	///
	/// assert_eq!(cache.resize(&1), Ok(_));
	///
	/// // Resizing to a size of zero will return a PaperError.
	/// assert_eq!(cache.resize(&0), Err(_));
	/// ```
	pub fn resize(&mut self, max_size: &CacheSize) -> Result<(), PaperError> {
		if *max_size == 0 {
			return Err(PaperError::new(
				ErrorKind::InvalidCacheSize,
				"The cache size cannot be zero."
			));
		}

		self.reduce(max_size)?;
		self.max_size = *max_size;

		Ok(())
	}

	fn reduce(&mut self, target_size: &CacheSize) -> Result<(), PaperError> {
		while self.current_size > *target_size {
			let policy_index = get_policy_index(self.policy);
			let policy_key = self.policy_stacks[policy_index].get_eviction();

			if let Some(key) = &policy_key {
				if let Err(_) = self.del(key) {
					return Err(PaperError::new(
						ErrorKind::Internal,
						"An internal error has occured."
					));
				}
			} else {
				return Err(PaperError::new(
					ErrorKind::Internal,
					"An internal error has occured."
				));
			}
		}

		Ok(())
	}
}

fn get_policy_index(policy: &Policy) -> usize {
	match policy {
		Policy::Lru => 0,
	}
}
