use std::sync::{Arc, Mutex};
use std::fmt::Display;
use std::hash::Hash;
use std::thread;
use crate::cache_error::CacheError;
use crate::object::MemSize;
use crate::stats::Stats;
use crate::policy::Policy;
use crate::cache::Cache;
use crate::worker::{Worker, TtlWorker};
pub use crate::cache::CacheSize;

pub struct PaperCache<K, V>
where
	K: 'static + Eq + Hash + Clone + Display + Sync,
	V: 'static + Clone + Sync + MemSize,
{
	cache: Arc<Mutex<Cache<K, V>>>,
}

impl<K, V> PaperCache<K, V>
where
	K: 'static + Eq + Hash + Clone + Display + Sync,
	V: 'static + Clone + Sync + MemSize,
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
	/// assert_eq!(Cache::<u32, u32>::new(100, Some(&[&Policy::Lru])), Ok(_));
	///
	/// // Supplying a maximum size of zero will return a CacheError.
	/// assert_eq!(Cache::<u32, u32>::new(0, Some(&[&Policy::Lru])), Err(_));
	///
	/// // Supplying an empty policies slice will return a CacheError.
	/// assert_eq!(Cache::<u32, u32>::new(0, Some(&[])), Err(_));
	/// ```
	pub fn new(
		max_size: CacheSize,
		policies: Option<Vec<&'static Policy>>
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

	/// Returns the current statistics.
	///
	/// # Examples
	/// ```
	/// let mut cache = PaperCache::<u32, u32>::new(100, None);
	///
	/// cache.set(0, 1, None);
	///
	/// assert_eq!(cache.stats().get_used_size(), 1);
	/// ```
	pub fn stats(&self) -> Stats {
		let cache = self.cache.lock().unwrap();
		cache.stats()
	}

	/// Gets the value associated with the supplied key.
	/// If the key was not found in the cache, returns a
	/// [`CacheError`].
	///
	/// # Examples
	/// ```
	/// let mut cache = PaperCache::<u32, u32>::new(100, None);
	///
	/// cache.set(0, 1, None);
	///
	/// // Getting a key which exists in the cache will return the associated value.
	/// assert_eq!(cache.get(0), Ok(1));
	/// // Getting a key which does not exist in the cache will return a CacheError.
	/// assert_eq!(cache.get(1), Err(_));
	/// ```
	pub fn get(&mut self, key: &K) -> Result<V, CacheError> {
		let mut cache = self.cache.lock().unwrap();
		cache.get(key)
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
	/// let mut cache = PaperCache::<u32, u32>::new(100, None);
	///
	/// assert_eq!(cache.set(0, 1, None), Ok(_));
	/// ```
	pub fn set(&mut self, key: K, value: V, ttl: Option<u32>) -> Result<(), CacheError> {
		let mut cache = self.cache.lock().unwrap();
		cache.set(key, value, ttl)
	}

	/// Deletes the object associated with the supplied key in the cache.
	/// Returns a [`CacheError`] if the key was not found in the cache.
	///
	/// # Examples
	/// ```
	/// let mut cache = PaperCache::<u32, u32>::new(100, None);
	///
	/// cache.set(0, 1, None);
	/// assert_eq!(cache.del(0), Ok(_));
	///
	/// // Deleting a key which does not exist in the cache will return a CacheError.
	/// assert_eq!(cache.del(1), Err(_));
	/// ```
	pub fn del(&mut self, key: &K) -> Result<(), CacheError> {
		let mut cache = self.cache.lock().unwrap();
		cache.del(key)
	}

	/// Deletes all objects in the cache and sets the cache's used size to zero.
	/// Returns a [`CacheError`] if the objects could not be cleared.
	///
	/// # Examples
	/// ```
	/// let mut cache = PaperCache::<u32, u32>::new(100, None);
	/// cache.clear();
	/// ```
	pub fn clear(&mut self) -> Result<(), CacheError> {
		let mut cache = self.cache.lock().unwrap();
		cache.clear()
	}

	/// Resizes the cache to the supplied maximum size.
	/// If the supplied size is zero, returns a [`CacheError`].
	///
	/// # Examples
	/// ```
	/// let mut cache = PaperCache::<u32, u32>::new(100, None);
	///
	/// assert_eq!(cache.resize(&1), Ok(_));
	///
	/// // Resizing to a size of zero will return a CacheError.
	/// assert_eq!(cache.resize(&0), Err(_));
	/// ```
	pub fn resize(&mut self, max_size: &CacheSize) -> Result<(), CacheError> {
		let mut cache = self.cache.lock().unwrap();
		cache.resize(max_size)
	}

	/// Sets the eviction policy of the cache to the supplied policy.
	/// If the supplied policy is not one of the considered eviction policies,
	/// a [`CacheError`] is returned.
	///
	/// # Examples
	/// ```
	/// let mut cache = PaperCache::<u32, u32>::new(100, Some(&[Policy::Lru]));
	///
	/// assert_eq!(cache.policy(&Policy::Lru), Ok(_));
	///
	/// // Supplying a policy that is not one of the considered policies will return a CacheError.
	/// assert_eq!(cache.policy(&Policy::Mru), Err(_));
	/// ```
	pub fn policy(&mut self, policy: &'static Policy) -> Result<(), CacheError> {
		let mut cache = self.cache.lock().unwrap();
		cache.policy(policy)
	}

	/// Registers a new background worker which implements [`Worker`].
	/// The worker will get a reference to the underlying cache.
	///
	/// # Examples
	/// ```
	/// let mut cache = PaperCache::<u32, u32>::new(100, None);
	/// cache.register_worker::<TtlWorker>();
	/// ```
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
	K: 'static + Eq + Hash + Clone + Display + Sync,
	V: 'static + Clone + Sync + MemSize,
{}
