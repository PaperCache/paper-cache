use std::{
	rc::Rc,
	sync::{Arc, Mutex},
	hash::Hash,
	thread,
};

use crate::{
	cache_error::CacheError,
	object::MemSize,
	stats::Stats,
	policy::Policy,
	cache::Cache,
	worker::{Worker, TtlWorker},
};

pub use crate::cache::CacheSize;

pub struct PaperCache<K, V>
where
	K: 'static + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{
	cache: Arc<Mutex<Cache<K, V>>>,
}

impl<K, V> PaperCache<K, V>
where
	K: 'static + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{
	/// Creates an empty PaperCache with maximum size `max_size`.
	/// If the maximum size is zero, a [`CacheError`] will be returned.
	/// The cache will only consider eviction policies specified
	/// by `policies` and return an error if the number of supplied
	/// `policies` is zero. If `None` is passed here, the cache
	/// will consider all eviction policies.
	///
	/// The cache's initial eviction policy will be the first policy or
	/// LFU if `None` is passed.
	///
	/// # Examples
	///
	/// ```
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize};
	///
	/// assert!(PaperCache::<u32, Object>::new(100, Some(vec![Policy::Lru])).is_ok());
	///
	/// // Supplying a maximum size of zero will return a CacheError.
	/// assert!(PaperCache::<u32, Object>::new(0, Some(vec![Policy::Lru])).is_err());
	///
	/// // Supplying an empty policies slice will return a CacheError.
	/// assert!(PaperCache::<u32, Object>::new(0, Some(vec![])).is_err());
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	pub fn new(
		max_size: CacheSize,
		policies: Option<Vec<Policy>>
	) -> Result<Self, CacheError> {
		let cache = Arc::new(Mutex::new(
			Cache::<K, V>::new(max_size, policies)?
		));

		let paper_cache = PaperCache {
			cache,
		};

		paper_cache.register_worker::<TtlWorker<K, V>>();

		Ok(paper_cache)
	}

	/// Returns the current cache version.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, ObjectMemSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, None).unwrap();
	/// assert_eq!(cache.version(), "0.1.0");
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	pub fn version(&self) -> String {
		env!("CARGO_PKG_VERSION").to_owned()
	}

	/// Returns the current statistics.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, ObjectMemSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, None).unwrap();
	///
	/// cache.set(0, Object, None);
	///
	/// assert_eq!(cache.stats().get_used_size(), 4);
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	pub fn stats(&self) -> Stats {
		self.cache
			.lock().unwrap()
			.stats()
	}

	/// Gets the value associated with the supplied key.
	/// If the key was not found in the cache, returns a [`CacheError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, ObjectMemSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, None).unwrap();
	///
	/// cache.set(0, Object, None);
	///
	/// // Getting a key which exists in the cache will return the associated value.
	/// assert!(cache.get(&0).is_ok());
	/// // Getting a key which does not exist in the cache will return a CacheError.
	/// assert!(cache.get(&1).is_err());
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	pub fn get(&mut self, key: &K) -> Result<Rc<V>, CacheError> {
		self.cache
			.lock().unwrap()
			.get(key)
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
	/// use paper_cache::{PaperCache, ObjectMemSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, None).unwrap();
	///
	/// assert!(cache.set(0, Object, None).is_ok());
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	pub fn set(&mut self, key: K, value: V, ttl: Option<u32>) -> Result<(), CacheError> {
		self.cache
			.lock().unwrap()
			.set(key, value, ttl)
	}

	/// Deletes the object associated with the supplied key in the cache.
	/// Returns a [`CacheError`] if the key was not found in the cache.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, ObjectMemSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, None).unwrap();
	///
	/// cache.set(0, Object, None);
	/// assert!(cache.del(&0).is_ok());
	///
	/// // Deleting a key which does not exist in the cache will return a CacheError.
	/// assert!(cache.del(&1).is_err());
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	pub fn del(&mut self, key: &K) -> Result<(), CacheError> {
		self.cache
			.lock().unwrap()
			.del(key)
	}

	/// Checks if an object with the supplied key exists in the cache without
	/// altering any of the cache's internal queues.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, ObjectMemSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, None).unwrap();
	///
	/// cache.set(0, Object, None);
	///
	/// assert!(cache.has(&0));
	/// assert!(!cache.has(&1));
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	pub fn has(&self, key: &K) -> bool {
		self.cache
			.lock().unwrap()
			.has(key)
	}

	/// Gets (peeks) the value associated with the supplied key without altering
	/// any of the cache's internal queues.
	/// If the key was not found in the cache, returns a [`CacheError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, ObjectMemSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(8, None).unwrap();
	///
	/// cache.set(0, Object, None);
	/// cache.set(1, Object, None);
	///
	/// // Peeking a key which exists in the cache will return the associated value.
	/// assert!(cache.peek(&0).is_ok());
	/// // Peeking a key which does not exist in the cache will return a CacheError.
	/// assert!(cache.peek(&2).is_err());
	///
	/// cache.set(2, Object, None);
	///
	/// // Peeking a key will not alter the eviction order of the objects.
	/// assert!(cache.peek(&1).is_ok());
	/// assert!(cache.peek(&2).is_ok());
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	pub fn peek(&self, key: &K) -> Result<Rc<V>, CacheError> {
		self.cache
			.lock().unwrap()
			.peek(key)
	}

	/// Deletes all objects in the cache and sets the cache's used size to zero.
	/// Returns a [`CacheError`] if the objects could not be wiped.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, ObjectMemSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, None).unwrap();
	/// cache.wipe();
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	pub fn wipe(&mut self) -> Result<(), CacheError> {
		self.cache
			.lock().unwrap()
			.wipe()
	}

	/// Resizes the cache to the supplied maximum size.
	/// If the supplied size is zero, returns a [`CacheError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, ObjectMemSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, None).unwrap();
	///
	/// assert!(cache.resize(1).is_ok());
	///
	/// // Resizing to a size of zero will return a CacheError.
	/// assert!(cache.resize(0).is_err());
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	pub fn resize(&mut self, max_size: CacheSize) -> Result<(), CacheError> {
		self.cache
			.lock().unwrap()
			.resize(max_size)
	}

	/// Sets the eviction policy of the cache to the supplied policy.
	/// If the supplied policy is not one of the considered eviction policies,
	/// a [`CacheError`] is returned.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, ObjectMemSize, Policy};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, Some(vec![Policy::Lru])).unwrap();
	///
	/// assert!(cache.policy(Policy::Lru).is_ok());
	///
	/// // Supplying a policy that is not one of the considered policies will return a CacheError.
	/// assert!(cache.policy(Policy::Mru).is_err());
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	pub fn policy(&mut self, policy: Policy) -> Result<(), CacheError> {
		self.cache
			.lock().unwrap()
			.policy(policy)
	}

	/// Registers a new background worker which implements [`Worker`].
	/// The worker will get a reference to the underlying cache.
	fn register_worker<T: Worker<K, V>>(&self) {
		let cache = Arc::clone(&self.cache);
		let worker = T::new(cache);

		thread::spawn(move || {
			worker.start();
		});
	}
}

unsafe impl<K, V> Send for PaperCache<K, V>
where
	K: 'static + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{}
