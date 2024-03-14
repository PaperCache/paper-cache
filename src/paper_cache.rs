use std::{
	sync::{
		Arc,
		atomic::AtomicU64,
	},
	hash::{Hash, BuildHasher},
	collections::hash_map::RandomState,
	thread,
};

use dashmap::DashMap;
use crossbeam_channel::unbounded;

use crate::{
	object::{Object, MemSize, ObjectSize},
	stats::{AtomicStats, Stats},
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
pub type AtomicCacheSize = AtomicU64;

pub type ObjectMapRef<K, V, S> = Arc<DashMap<K, Object<V>, S>>;
pub type StatsRef = Arc<AtomicStats>;

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
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize, ObjectSize};
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
	///     fn mem_size(&self) -> ObjectSize { 4 }
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
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize, ObjectSize};
	///
	/// assert!(PaperCache::<u32, Object>::with_hasher(100, &[Policy::Lru], RandomState::default()).is_ok());
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> ObjectSize { 4 }
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
		let stats = Arc::new(AtomicStats::new(max_size, 0));

		let (policy_worker, policy_listener) = unbounded();
		let (ttl_worker, ttl_listener) = unbounded();

		register_worker(PolicyWorker::<K, V, S>::new(
			policy_listener,
			objects.clone(),
			stats.clone(),
			policies.into(),
		));

		register_worker(TtlWorker::<K, V, S>::new(
			ttl_listener,
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
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize, ObjectSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, &[Policy::Lfu]).unwrap();
	/// assert_eq!(cache.version(), "1.2.4");
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> ObjectSize { 4 }
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
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize, ObjectSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, &[Policy::Lfu]).unwrap();
	///
	/// cache.set(0, Object, None);
	///
	/// assert_eq!(cache.stats().get_used_size(), 4);
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> ObjectSize { 4 }
	/// }
	/// ```
	pub fn stats(&self) -> Stats {
		self.stats.to_stats(self.policies.clone())
	}

	/// Gets the value associated with the supplied key.
	/// If the key was not found in the cache, returns a [`CacheError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize, ObjectSize};
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
	///     fn mem_size(&self) -> ObjectSize { 4 }
	/// }
	/// ```
	pub fn get(&self, key: K) -> Result<Arc<V>, CacheError> {
		let result = match self.objects.get(&key) {
			Some(object) if !object.is_expired() => {
				self.stats.hit();
				Ok(object.data())
			},

			_ => {
				self.stats.miss();
				Err(CacheError::KeyNotFound)
			},
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
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize, ObjectSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, &[Policy::Lfu]).unwrap();
	///
	/// assert!(cache.set(0, Object, None).is_ok());
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> ObjectSize { 4 }
	/// }
	/// ```
	pub fn set(&self, key: K, value: V, ttl: Option<u32>) -> Result<(), CacheError> {
		let object = Object::new(value, ttl);
		let size = object.size();
		let expiry = object.expiry();

		if size == 0 {
			return Err(CacheError::ZeroValueSize);
		}

		if self.stats.exceeds_max_size(size) {
			return Err(CacheError::ExceedingValueSize);
		}

		self.stats.set();

		if let Some(old_object) = self.objects.insert(key, object) {
			self.stats.decrease_used_size(old_object.size());
		}

		self.stats.increase_used_size(size);

		self.broadcast(WorkerEvent::Set(key, size, expiry))?;

		Ok(())
	}

	/// Deletes the object associated with the supplied key in the cache.
	/// Returns a [`CacheError`] if the key was not found in the cache.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize, ObjectSize};
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
	///     fn mem_size(&self) -> ObjectSize { 4 }
	/// }
	/// ```
	pub fn del(&self, key: K) -> Result<(), CacheError> {
		let object = erase(&self.objects, &self.stats, key)?;

		self.stats.del();
		self.broadcast(WorkerEvent::Del(key, object.expiry()))?;

		Ok(())
	}

	/// Checks if an object with the supplied key exists in the cache without
	/// altering any of the cache's internal queues.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize, ObjectSize};
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
	///     fn mem_size(&self) -> ObjectSize { 4 }
	/// }
	/// ```
	pub fn has(&self, key: K) -> bool {
		self.objects.get(&key).is_some_and(|object| !object.is_expired())
	}

	/// Gets (peeks) the value associated with the supplied key without altering
	/// any of the cache's internal queues.
	/// If the key was not found in the cache, returns a [`CacheError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize, ObjectSize};
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
	///     fn mem_size(&self) -> ObjectSize { 4 }
	/// }
	/// ```
	pub fn peek(&self, key: K) -> Result<Arc<V>, CacheError> {
		match self.objects.get(&key) {
			Some(object) if !object.is_expired() => Ok(object.data()),
			_ => Err(CacheError::KeyNotFound),
		}
	}

	/// Sets the TTL associated with the supplied key.
	/// If the key was not found in the cache, returns a [`CacheError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize, ObjectSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(8, &[Policy::Lfu]).unwrap();
	///
	/// cache.set(0, Object, None); // value will not expire
	/// cache.ttl(0, Some(5)); // value will expire in 5 seconds
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> ObjectSize { 4 }
	/// }
	/// ```
	pub fn ttl(&self, key: K, ttl: Option<u32>) -> Result<(), CacheError> {
		let mut object = match self.objects.get_mut(&key) {
			Some(object) if !object.is_expired() => object,
			_ => return Err(CacheError::KeyNotFound),
		};

		let old_expiry = object.expiry();
		object.expires(ttl);
		let new_expiry = object.expiry();

		self.broadcast(WorkerEvent::Ttl(key, old_expiry, new_expiry))?;

		Ok(())
	}

	/// Gets the size of the value associated with the supplied key in bytes.
	/// If the key was not found in the cache, returns a [`CacheError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize, ObjectSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, &[Policy::Lfu]).unwrap();
	///
	/// cache.set(0, Object, None);
	///
	/// // Sizing a key which exists in the cache will return the size of the associated value.
	/// assert_eq!(cache.size(0), Ok(4));
	/// // Sizing a key which does not exist in the cache will return a CacheError.
	/// assert!(cache.size(1).is_err());
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> ObjectSize { 4 }
	/// }
	/// ```
	pub fn size(&self, key: K) -> Result<ObjectSize, CacheError> {
		self.peek(key).map(|value| value.mem_size())
	}

	/// Deletes all objects in the cache and sets the cache's used size to zero.
	/// Returns a [`CacheError`] if the objects could not be wiped.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize, ObjectSize};
	///
	/// let mut cache = PaperCache::<u32, Object>::new(100, &[Policy::Lfu]).unwrap();
	/// cache.wipe();
	///
	/// struct Object;
	///
	/// impl ObjectMemSize for Object {
	///     fn mem_size(&self) -> ObjectSize { 4 }
	/// }
	/// ```
	pub fn wipe(&self) -> Result<(), CacheError> {
		self.objects.clear();

		self.stats.reset_used_size();
		self.broadcast(WorkerEvent::Wipe)?;

		Ok(())
	}

	/// Resizes the cache to the supplied maximum size.
	/// If the supplied size is zero, returns a [`CacheError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize, ObjectSize};
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
	///     fn mem_size(&self) -> ObjectSize { 4 }
	/// }
	/// ```
	pub fn resize(&self, max_size: CacheSize) -> Result<(), CacheError> {
		if max_size == 0 {
			return Err(CacheError::ZeroCacheSize);
		}

		self.stats.set_max_size(max_size);
		self.broadcast(WorkerEvent::Resize(max_size))?;

		Ok(())
	}

	/// Sets the eviction policy of the cache to the supplied policy.
	/// If the supplied policy is not one of the considered eviction policies,
	/// a [`CacheError`] is returned.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, Policy, ObjectMemSize, ObjectSize};
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
	///     fn mem_size(&self) -> ObjectSize { 4 }
	/// }
	/// ```
	pub fn policy(&self, policy: Policy) -> Result<(), CacheError> {
		let index = self.policies
			.iter()
			.position(|stored_policy| stored_policy.eq(&policy));

		let Some(index) = index else {
			return Err(CacheError::UnconfiguredPolicy);
		};

		self.stats.set_policy_index(index);
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
fn register_worker<K, V, S>(mut worker: impl Worker<K, V, S>)
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
	S: Default + Clone + BuildHasher,
{
	thread::spawn(move || worker.run());
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

	stats.decrease_used_size(object.size());

	match !object.is_expired() {
		true => Ok(object),
		false => Err(CacheError::KeyNotFound),
	}
}

unsafe impl<K, V, S> Send for PaperCache<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
	S: Default + Clone + BuildHasher,
{}
