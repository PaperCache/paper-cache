use std::{
	sync::{Arc, RwLock},
	hash::{Hash, BuildHasher},
	collections::hash_map::RandomState,
	thread,
};

use dashmap::DashMap;
use crossbeam_channel::unbounded;

use crate::{
	object::{Object, MemSize},
	stats::Stats,
	policy::Policy,
	error::CacheError,
	worker::{
		Worker,
		WorkerSender,
		WorkerEvent,
		PolicyWorker,
		TtlWorker,
	},
};

pub type CacheSize = u64;

pub type ObjectMapRef<K, V, S> = Arc<DashMap<K, Object<V>, S>>;
pub type StatsRef = Arc<RwLock<Stats>>;

pub struct PaperCache<K, V, S = RandomState>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
	S: Default + BuildHasher+ Clone,
{
	objects: ObjectMapRef<K, V, S>,
	stats: StatsRef,

	policies: Arc<Box<[Policy]>>,
	workers: Arc<Box<[WorkerSender<K>]>>,
}

impl<K, V, S> PaperCache<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
	S: 'static + Default + Clone + BuildHasher,
{
	/// Creates an empty `PaperCache` with maximum size `max_size`.
	/// If the maximum size is zero, a [`CacheError`] will be returned.
	/// The cache will only consider eviction policies specified
	/// by `policies` and return an error if the number of supplied
	/// `policies` is zero. The cache's initial eviction policy will
	/// be the first policy.
	///
	/// # Examples
	///
	/// ```
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize};
	///
	/// assert!(PaperCache::<u32, Object>::new(100, &[Policy::Lru]).is_ok());
	///
	/// // Supplying a maximum size of zero will return a CacheError.
	/// assert!(PaperCache::<u32, Object>::new(0, &[Policy::Lru]).is_err());
	///
	/// // Supplying an empty policies slice will return a CacheError.
	/// assert!(PaperCache::<u32, Object>::new(0, &[]).is_err());
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	pub fn new(
		max_size: CacheSize,
		policies: &[Policy],
	) -> Result<Self, CacheError> {
		Self::with_hasher(max_size, policies, Default::default())
	}

	/// Creates an empty `PaperCache` with the supplied hasher.
	///
	/// # Examples
	///
	/// ```
	/// use std::collections::hash_map::RandomState;
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize};
	///
	/// assert!(PaperCache::<u32, Object>::with_hasher(100, &[Policy::Lru], RandomState::default()).is_ok());
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	pub fn with_hasher(
		max_size: CacheSize,
		policies: &[Policy],
		hasher: S,
	) -> Result<Self, CacheError> {
		if max_size == 0 {
			return Err(CacheError::ZeroCacheSize);
		}

		if policies.is_empty() {
			return Err(CacheError::EmptyPolicies);
		}

		let objects = Arc::new(DashMap::with_hasher(hasher));
		let stats = Arc::new(RwLock::new(Stats::new(max_size, policies[0])));

		let policy_worker = register_worker(PolicyWorker::<K, V, S>::new(
			objects.clone(),
			stats.clone(),
			policies.into(),
		));

		let ttl_worker = register_worker(TtlWorker::<K, V, S>::new(
			objects.clone(),
			stats.clone(),
		));

		let cache = PaperCache {
			objects,
			stats,

			policies: Arc::new(policies.into()),
			workers: Arc::new(Box::new([policy_worker, ttl_worker])),
		};

		Ok(cache)
	}

	/// Returns the current cache version.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, &[Policy::Lfu]).unwrap();
	/// assert_eq!(cache.version(), "0.1.0");
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	#[must_use]
	pub fn version(&self) -> String {
		env!("CARGO_PKG_VERSION").to_owned()
	}

	/// Returns the current statistics.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, &[Policy::Lfu]).unwrap();
	///
	/// cache.set(0, Object, None);
	///
	/// assert_eq!(cache.stats().unwrap().get_used_size(), 4);
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	pub fn stats(&self) -> Result<Stats, CacheError> {
		let stats = self.stats
			.read().map_err(|_| CacheError::Internal)?
			.clone();

		Ok(stats)
	}

	/// Gets the value associated with the supplied key.
	/// If the key was not found in the cache, returns a [`CacheError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, &[Policy::Lfu]).unwrap();
	///
	/// cache.set(0, Object, None);
	///
	/// // Getting a key which exists in the cache will return the associated value.
	/// assert!(cache.get(0).is_ok());
	/// // Getting a key which does not exist in the cache will return a CacheError.
	/// assert!(cache.get(1).is_err());
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	pub fn get(&self, key: K) -> Result<Arc<V>, CacheError> {
		let result = match self.objects.get(&key) {
			Some(entry) => {
				self.stats
					.write().map_err(|_| CacheError::Internal)?
					.hit();

				Ok(entry.data())
			},

			None => {
				self.stats
					.write().map_err(|_| CacheError::Internal)?
					.miss();

				Err(CacheError::KeyNotFound)
			}
		};

		self.broadcast(WorkerEvent::Get(key))?;

		result
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
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, &[Policy::Lfu]).unwrap();
	///
	/// assert!(cache.set(0, Object, None).is_ok());
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	pub fn set(&self, key: K, value: V, ttl: Option<u32>) -> Result<(), CacheError> {
		let object = Object::new(value, ttl);
		let size = object.size();
		let expiry = object.expiry();

		if size == 0 {
			return Err(CacheError::ZeroValueSize);
		}

		let mut stats = self.stats.write().map_err(|_| CacheError::Internal)?;

		if stats.exceeds_max_size(size) {
			return Err(CacheError::ExceedingValueSize);
		}

		stats.set();

		if let Some(old_object) = self.objects.insert(key, object) {
			stats.decrease_used_size(old_object.size());
		}

		stats.increase_used_size(size);

		self.broadcast(WorkerEvent::Set(key, size, expiry))?;

		Ok(())
	}

	/// Deletes the object associated with the supplied key in the cache.
	/// Returns a [`CacheError`] if the key was not found in the cache.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, &[Policy::Lfu]).unwrap();
	///
	/// cache.set(0, Object, None);
	/// assert!(cache.del(0).is_ok());
	///
	/// // Deleting a key which does not exist in the cache will return a CacheError.
	/// assert!(cache.del(1).is_err());
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	pub fn del(&self, key: K) -> Result<(), CacheError> {
		let object = erase(&self.objects, &self.stats, key)?;

		self.stats
			.write().map_err(|_| CacheError::Internal)?
			.del();

		self.broadcast(WorkerEvent::Del(key, object.expiry()))?;

		Ok(())
	}

	/// Checks if an object with the supplied key exists in the cache without
	/// altering any of the cache's internal queues.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, &[Policy::Lfu]).unwrap();
	///
	/// cache.set(0, Object, None);
	///
	/// assert!(cache.has(0));
	/// assert!(!cache.has(1));
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	pub fn has(&self, key: K) -> bool {
		self.objects.contains_key(&key)
	}

	/// Gets (peeks) the value associated with the supplied key without altering
	/// any of the cache's internal queues.
	/// If the key was not found in the cache, returns a [`CacheError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(8, &[Policy::Lfu]).unwrap();
	///
	/// cache.set(0, Object, None);
	/// cache.set(1, Object, None);
	///
	/// // Peeking a key which exists in the cache will return the associated value.
	/// assert!(cache.peek(0).is_ok());
	/// // Peeking a key which does not exist in the cache will return a CacheError.
	/// assert!(cache.peek(2).is_err());
	///
	/// cache.set(2, Object, None);
	///
	/// // Peeking a key will not alter the eviction order of the objects.
	/// assert!(cache.peek(1).is_ok());
	/// assert!(cache.peek(2).is_ok());
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	pub fn peek(&self, key: K) -> Result<Arc<V>, CacheError> {
		self.objects.get(&key)
			.map(|object| object.data())
			.ok_or(CacheError::KeyNotFound)
	}

	/// Deletes all objects in the cache and sets the cache's used size to zero.
	/// Returns a [`CacheError`] if the objects could not be wiped.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, &[Policy::Lfu]).unwrap();
	/// cache.wipe();
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> usize { 4 }
	/// }
	/// ```
	pub fn wipe(&self) -> Result<(), CacheError> {
		self.objects.clear();

		self.stats
			.write().map_err(|_| CacheError::Internal)?
			.reset_used_size();

		self.broadcast(WorkerEvent::Wipe)?;

		Ok(())
	}

	/// Resizes the cache to the supplied maximum size.
	/// If the supplied size is zero, returns a [`CacheError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, &[Policy::Lfu]).unwrap();
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
	pub fn resize(&self, max_size: CacheSize) -> Result<(), CacheError> {
		if max_size == 0 {
			return Err(CacheError::ZeroCacheSize);
		}

		self.stats
			.write().map_err(|_| CacheError::Internal)?
			.set_max_size(max_size);

		self.broadcast(WorkerEvent::Resize(max_size))?;

		Ok(())
	}

	/// Sets the eviction policy of the cache to the supplied policy.
	/// If the supplied policy is not one of the considered eviction policies,
	/// a [`CacheError`] is returned.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, &[Policy::Lru]).unwrap();
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
	pub fn policy(&self, policy: Policy) -> Result<(), CacheError> {
		if !self.policies.contains(&policy) {
			return Err(CacheError::UnconfiguredPolicy);
		}

		let mut stats = self.stats.write().map_err(|_| CacheError::Internal)?;
		stats.set_policy(policy);

		self.broadcast(WorkerEvent::Policy(policy))?;

		Ok(())
	}

	fn broadcast(&self, event: WorkerEvent<K>) -> Result<(), CacheError> {
		for worker in self.workers.iter() {
			worker.send(event.clone()).map_err(|_| CacheError::Internal)?;
		}

		Ok(())
	}
}

/// Registers a new background worker which implements [`Worker`].
fn register_worker<K, V, S>(mut worker: impl Worker<K, V, S>) -> WorkerSender<K>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
	S: Default + Clone + BuildHasher,
{
	let (sender, receiver) = unbounded();
	worker.listen(receiver);

	thread::spawn(move || worker.run());

	sender
}

pub fn erase<K, V, S>(
	objects: &ObjectMapRef<K, V, S>,
	stats: &StatsRef,
	key: K,
) -> Result<Object<V>, CacheError>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
	S: Default + Clone + BuildHasher,
{
	let (_, object) = objects
		.remove(&key)
		.ok_or(CacheError::KeyNotFound)?;

	let mut stats = stats.write().map_err(|_| CacheError::Internal)?;

	stats.decrease_used_size(object.size());

	Ok(object)
}

unsafe impl<K, V, S> Send for PaperCache<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
	S: Default + Clone + BuildHasher,
{}
