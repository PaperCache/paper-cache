use std::{
	thread,
	sync::{
		Arc,
		atomic::AtomicU64,
	},
	hash::{
		Hash,
		RandomState,
		BuildHasher,
		BuildHasherDefault,
	},
};

use dashmap::{
	DashMap,
	mapref::entry::Entry,
};

use typesize::TypeSize;
use nohash_hasher::NoHashHasher;
use crossbeam_channel::unbounded;
use log::info;
use kwik::fmt;

use crate::{
	stats::{AtomicStats, Stats},
	policy::PaperPolicy,
	error::CacheError,
	object::{
		Object,
		ObjectSize,
		overhead::OverheadManager,
	},
	worker::{
		Worker,
		WorkerSender,
		WorkerEvent,
		WorkerManager,
	},
};

pub type CacheSize = u64;
pub type AtomicCacheSize = AtomicU64;

pub type HashedKey = u64;
pub type NoHasher = BuildHasherDefault<NoHashHasher<HashedKey>>;

pub type ObjectMapRef<K, V> = Arc<DashMap<HashedKey, Object<K, V>, NoHasher>>;
pub type StatsRef = Arc<AtomicStats>;
pub type OverheadManagerRef = Arc<OverheadManager>;

pub const POLICIES: &[PaperPolicy] = &[
	PaperPolicy::Lfu,
	PaperPolicy::Fifo,
	PaperPolicy::Lru,
	PaperPolicy::Mru,
];

pub struct PaperCache<K, V, S = RandomState> {
	objects: ObjectMapRef<K, V>,
	stats: StatsRef,

	worker_manager: Arc<WorkerSender>,
	overhead_manager: OverheadManagerRef,

	hasher: S,
}

impl<K, V, S> PaperCache<K, V, S>
where
	K: 'static + Eq + Hash + TypeSize,
	V: 'static + TypeSize,
	S: Default + Clone + BuildHasher,
{
	/// Creates an empty `PaperCache` with maximum size `max_size` and
	/// eviction policy `policy`. If the maximum size is zero, a
	/// [`CacheError`] will be returned.
	///
	/// # Examples
	///
	/// ```
	/// use paper_cache::{PaperCache, PaperPolicy};
	///
	/// assert!(PaperCache::<u32, u32>::new(1000, PaperPolicy::Lru).is_ok());
	///
	/// // Supplying a maximum size of zero will return a CacheError.
	/// assert!(PaperCache::<u32, u32>::new(0, PaperPolicy::Lru).is_err());
	/// ```
	pub fn new(
		max_size: CacheSize,
		policy: PaperPolicy,
	) -> Result<Self, CacheError> {
		Self::with_hasher(max_size, policy, Default::default())
	}

	/// Creates an empty `PaperCache` with the supplied hasher.
	///
	/// # Examples
	///
	/// ```
	/// use std::hash::RandomState;
	/// use paper_cache::{PaperCache, PaperPolicy};
	///
	/// assert!(PaperCache::<u32, u32>::with_hasher(1000, PaperPolicy::Lru, RandomState::default()).is_ok());
	/// ```
	pub fn with_hasher(
		max_size: CacheSize,
		policy: PaperPolicy,
		hasher: S,
	) -> Result<Self, CacheError>
	{
		if max_size == 0 {
			return Err(CacheError::ZeroCacheSize);
		}

		let objects = Arc::new(DashMap::with_hasher(NoHasher::default()));
		let stats = Arc::new(AtomicStats::new(max_size, policy)?);

		let overhead_manager = Arc::new(OverheadManager::default());

		let (worker_sender, worker_listener) = unbounded();

		let mut worker_manager = WorkerManager::new(
			worker_listener,
			&objects,
			&stats,
			&overhead_manager,
			policy,
		);

		thread::spawn(move || worker_manager.run());

		let cache = PaperCache {
			objects,
			stats,

			worker_manager: Arc::new(worker_sender),
			overhead_manager,

			hasher,
		};

		Ok(cache)
	}

	/// Returns the current cache version.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, PaperPolicy};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(1000, PaperPolicy::Lfu).unwrap();
	/// assert_eq!(cache.version(), env!("CARGO_PKG_VERSION"));
	/// ```
	#[must_use]
	pub fn version(&self) -> String {
		env!("CARGO_PKG_VERSION").to_owned()
	}

	/// Returns the current statistics.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, PaperPolicy};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(1000, PaperPolicy::Lfu).unwrap();
	///
	/// cache.set(0, 0, None);
	///
	/// assert!(cache.stats().get_used_size() > 0);
	/// ```
	#[must_use]
	pub fn stats(&self) -> Stats {
		self.stats.to_stats()
	}

	/// Gets the value associated with the supplied key.
	/// If the key was not found in the cache, returns a [`CacheError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, PaperPolicy};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(1000, PaperPolicy::Lfu).unwrap();
	///
	/// cache.set(0, 0, None);
	///
	/// // Getting a key which exists in the cache will return the associated value.
	/// assert!(cache.get(0).is_ok());
	/// // Getting a key which does not exist in the cache will return a CacheError.
	/// assert!(cache.get(1).is_err());
	/// ```
	pub fn get(&self, key: &K) -> Result<Arc<V>, CacheError> {
		let hashed_key = self.hash_key(key);

		let result = match self.objects.get(&hashed_key) {
			Some(object) if object.key_matches(key) && !object.is_expired() => {
				self.stats.hit();
				Ok(object.data())
			},

			_ => {
				self.stats.miss();
				Err(CacheError::KeyNotFound)
			},
		};

		self.broadcast(WorkerEvent::Get(hashed_key, result.is_ok()))?;

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
	/// use paper_cache::{PaperCache, PaperPolicy};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(1000, PaperPolicy::Lfu).unwrap();
	///
	/// assert!(cache.set(0, 0, None).is_ok());
	/// ```
	pub fn set(&self, key: K, value: V, ttl: Option<u32>) -> Result<(), CacheError> {
		let hashed_key = self.hash_key(&key);

		let object = Object::new(key, value, ttl);
		let size = self.overhead_manager.total_size(&object);
		let expiry = object.expiry();

		if size == 0 {
			return Err(CacheError::ZeroValueSize);
		}

		if self.stats.exceeds_max_size(size.into()) {
			return Err(CacheError::ExceedingValueSize);
		}

		self.stats.set();

		let old_object_info = self.objects
			.insert(hashed_key, object)
			.map(|old_object| {
				let size = self.overhead_manager.total_size(&old_object);
				let expiry = old_object.expiry();

				(size, expiry)
			});

		self.stats.increase_used_size(size.into());

		if let Some((old_object_size, _)) = old_object_info {
			self.stats.decrease_used_size(old_object_size.into());
		}

		self.broadcast(WorkerEvent::Set(hashed_key, size, expiry, old_object_info))?;

		Ok(())
	}

	/// Deletes the object associated with the supplied key in the cache.
	/// Returns a [`CacheError`] if the key was not found in the cache.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, PaperPolicy};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(1000, PaperPolicy::Lfu).unwrap();
	///
	/// cache.set(0, 0, None);
	/// assert!(cache.del(0).is_ok());
	///
	/// // Deleting a key which does not exist in the cache will return a CacheError.
	/// assert!(cache.del(1).is_err());
	/// ```
	pub fn del(&self, key: &K) -> Result<(), CacheError> {
		let hashed_key = self.hash_key(key);

		let (removed_hashed_key, object) = erase(
			&self.objects,
			&self.stats,
			&self.overhead_manager,
			Some(EraseKey::Original(key, hashed_key)),
		)?;

		self.stats.del();

		self.broadcast(WorkerEvent::Del(
			removed_hashed_key,
			self.overhead_manager.total_size(&object),
			object.expiry(),
		))?;

		Ok(())
	}

	/// Checks if an object with the supplied key exists in the cache without
	/// altering any of the cache's internal queues.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, PaperPolicy};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(1000, PaperPolicy::Lfu).unwrap();
	///
	/// cache.set(0, 0, None);
	///
	/// assert!(cache.has(0));
	/// assert!(!cache.has(1));
	/// ```
	pub fn has(&self, key: &K) -> bool {
		let hashed_key = self.hash_key(key);

		self.objects
			.get(&hashed_key)
			.is_some_and(|object| object.key_matches(key) && !object.is_expired())
	}

	/// Gets (peeks) the value associated with the supplied key without altering
	/// any of the cache's internal queues.
	/// If the key was not found in the cache, returns a [`CacheError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, PaperPolicy};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(1000, PaperPolicy::Lfu).unwrap();
	///
	/// cache.set(0, 0, None);
	/// cache.set(1, 0, None);
	///
	/// // Peeking a key which exists in the cache will return the associated value.
	/// assert!(cache.peek(0).is_ok());
	/// // Peeking a key which does not exist in the cache will return a CacheError.
	/// assert!(cache.peek(2).is_err());
	///
	/// cache.set(2, 0, None);
	///
	/// // Peeking a key will not alter the eviction order of the objects.
	/// assert!(cache.peek(1).is_ok());
	/// assert!(cache.peek(2).is_ok());
	/// ```
	pub fn peek(&self, key: &K) -> Result<Arc<V>, CacheError> {
		let hashed_key = self.hash_key(key);

		match self.objects.get(&hashed_key) {
			Some(object) if object.key_matches(key) && !object.is_expired() =>
				Ok(object.data()),

			_ => Err(CacheError::KeyNotFound),
		}
	}

	/// Sets the TTL associated with the supplied key.
	/// If the key was not found in the cache, returns a [`CacheError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, PaperPolicy};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(1000, PaperPolicy::Lfu).unwrap();
	///
	/// cache.set(0, 0, None); // value will not expire
	/// cache.ttl(0, Some(5)); // value will expire in 5 seconds
	/// ```
	pub fn ttl(&self, key: &K, ttl: Option<u32>) -> Result<(), CacheError> {
		let hashed_key = self.hash_key(key);

		let mut object = match self.objects.get_mut(&hashed_key) {
			Some(object) if object.key_matches(key) && !object.is_expired() => object,
			_ => return Err(CacheError::KeyNotFound),
		};

		let old_expiry = object.expiry();
		object.expires(ttl);
		let new_expiry = object.expiry();

		self.broadcast(WorkerEvent::Ttl(hashed_key, old_expiry, new_expiry))?;

		Ok(())
	}

	/// Gets the size of the value associated with the supplied key in bytes.
	/// If the key was not found in the cache, returns a [`CacheError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, PaperPolicy};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(1000, PaperPolicy::Lfu).unwrap();
	///
	/// cache.set(0, 0, None);
	///
	/// // Sizing a key which exists in the cache will return the size of the associated value.
	/// assert!(cache.size(0).is_ok());
	/// // Sizing a key which does not exist in the cache will return a CacheError.
	/// assert!(cache.size(1).is_err());
	/// ```
	pub fn size(&self, key: &K) -> Result<ObjectSize, CacheError> {
		let hashed_key = self.hash_key(key);

		match self.objects.get(&hashed_key) {
			Some(object) if object.key_matches(key) && !object.is_expired() =>
				Ok(self.overhead_manager.total_size(&object)),

			_ => Err(CacheError::KeyNotFound),
		}
	}

	/// Deletes all objects in the cache and sets the cache's used size to zero.
	/// Returns a [`CacheError`] if the objects could not be wiped.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, PaperPolicy};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(1000, PaperPolicy::Lfu).unwrap();
	/// cache.wipe();
	/// ```
	pub fn wipe(&self) -> Result<(), CacheError> {
		info!("Wiping cache");

		self.objects.clear();
		self.stats.clear();

		self.broadcast(WorkerEvent::Wipe)?;

		Ok(())
	}

	/// Resizes the cache to the supplied maximum size.
	/// If the supplied size is zero, returns a [`CacheError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, PaperPolicy};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(1000, PaperPolicy::Lfu).unwrap();
	///
	/// assert!(cache.resize(1).is_ok());
	///
	/// // Resizing to a size of zero will return a CacheError.
	/// assert!(cache.resize(0).is_err());
	/// ```
	pub fn resize(&self, max_size: CacheSize) -> Result<(), CacheError> {
		if max_size == 0 {
			return Err(CacheError::ZeroCacheSize);
		}

		let current_max_size = self.stats.get_max_size();

		if max_size == current_max_size {
			return Ok(());
		}

		info!(
			"Resizing cache from {} to {}",
			fmt::memory(current_max_size, Some(2)),
			fmt::memory(max_size, Some(2)),
		);

		self.stats.set_max_size(max_size);
		self.broadcast(WorkerEvent::Resize(max_size))?;

		Ok(())
	}

	/// Sets the eviction policy of the cache to the supplied policy.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, PaperPolicy};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(1000, PaperPolicy::Lru).unwrap();
	///
	/// assert!(cache.policy(PaperPolicy::Lru).is_ok());
	/// ```
	pub fn policy(&self, policy: PaperPolicy) -> Result<(), CacheError> {
		self.stats.set_policy(policy)?;
		self.broadcast(WorkerEvent::Policy(policy))?;

		Ok(())
	}

	fn broadcast(&self, event: WorkerEvent) -> Result<(), CacheError> {
		self.worker_manager
			.try_send(event)
			.map_err(|_| CacheError::Internal)?;

		Ok(())
	}

	fn hash_key(&self, key: &K) -> HashedKey {
		self.hasher.hash_one(key)
	}
}

pub enum EraseKey<'a, K> {
	Original(&'a K, HashedKey),
	Hashed(HashedKey),
}

pub fn erase<K, V>(
	objects: &ObjectMapRef<K, V>,
	stats: &StatsRef,
	overhead_manager: &OverheadManagerRef,
	maybe_key: Option<EraseKey<K>>,
) -> Result<(HashedKey, Object<K, V>), CacheError>
where
	K: Eq + TypeSize,
	V: TypeSize,
{
	let hashed_key = match maybe_key {
		Some(EraseKey::Original(_, hashed_key)) => hashed_key,
		Some(EraseKey::Hashed(hashed_key)) => hashed_key,

		None => {
			// the policy has run out of keys to evict (either it's a mini stack or
			// something went wrong during policy reconstruction) so we fall back
			// to evicting a random object

			objects
				.iter()
				.next()
				.ok_or(CacheError::Internal)?
				.key().to_owned()
		},
	};

	// don't remove the object right away because if we have the original key,
	// we need to do a validation check that it matches the object's key in
	// case of a hash collision
	let Entry::Occupied(entry) = objects.entry(hashed_key) else {
		return Err(CacheError::KeyNotFound);
	};

	if let Some(EraseKey::Original(key, _)) = maybe_key {
		if !entry.get().key_matches(key) {
			return Err(CacheError::KeyNotFound);
		}
	};

	let object = entry.remove();

	stats.decrease_used_size(overhead_manager.total_size(&object).into());

	match !object.is_expired() {
		true => Ok((hashed_key, object)),
		false => Err(CacheError::KeyNotFound),
	}
}

unsafe impl<K, V, S> Send for PaperCache<K, V, S> {}
