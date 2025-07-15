/*
 * Copyright (c) Kia Shakiba
 *
 * This source code is licensed under the GNU AGPLv3 license found in the
 * LICENSE file in the root directory of this source tree.
 */

mod error;
mod worker;
mod object;
mod policy;
mod status;

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

use kwik::{
	fmt,
	math::set::Multiset,
};

use crate::{
	status::{AtomicStatus, Status},
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

pub use crate::{
	error::CacheError,
	policy::PaperPolicy,
};

pub type CacheSize = u64;
pub type AtomicCacheSize = AtomicU64;

pub type HashedKey = u64;
pub type NoHasher = BuildHasherDefault<NoHashHasher<HashedKey>>;

pub type ObjectMapRef<K, V> = Arc<DashMap<HashedKey, Object<K, V>, NoHasher>>;
pub type StatusRef = Arc<AtomicStatus>;
pub type OverheadManagerRef = Arc<OverheadManager>;

pub struct PaperCache<K, V, S = RandomState> {
	objects: ObjectMapRef<K, V>,
	status: StatusRef,

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
	/// let cache = PaperCache::<u32, u32>::new(
	///     1000,
	///     &[PaperPolicy::Lfu],
	///     PaperPolicy::Lfu,
	/// );
	///
	/// assert!(cache.is_ok());
	///
	/// // Supplying a maximum size of zero will return a `CacheError`.
	/// let cache = PaperCache::<u32, u32>::new(
	///     0,
	///     &[PaperPolicy::Lfu],
	///     PaperPolicy::Lfu,
	/// );
	///
	/// assert!(cache.is_err());
	///
	/// // Supplying duplicate policies will return a `CacheError`.
	/// let cache = PaperCache::<u32, u32>::new(
	///     1000,
	///     &[PaperPolicy::Lfu, PaperPolicy::Lru, PaperPolicy::Lfu],
	///     PaperPolicy::Lfu,
	/// );
	///
	/// assert!(cache.is_err());
	///
	/// // Supplying a non-configured policy will return a `CacheError`.
	/// let cache = PaperCache::<u32, u32>::new(
	///     1000,
	///     &[PaperPolicy::Lfu],
	///     PaperPolicy::Lru,
	/// );
	///
	/// assert!(cache.is_err());
	/// ```
	pub fn new(
		max_size: CacheSize,
		policies: &[PaperPolicy],
		policy: PaperPolicy,
	) -> Result<Self, CacheError> {
		Self::with_hasher(
			max_size,
			policies,
			policy,
			Default::default(),
		)
	}

	/// Creates an empty `PaperCache` with the supplied hasher.
	///
	/// # Examples
	///
	/// ```
	/// use std::hash::RandomState;
	/// use paper_cache::{PaperCache, PaperPolicy};
	///
	/// let cache = PaperCache::<u32, u32>::with_hasher(
	///     1000,
	///     &[PaperPolicy::Lfu],
	///     PaperPolicy::Lfu,
	///     RandomState::default(),
	/// );
	///
	/// assert!(cache.is_ok());
	/// ```
	pub fn with_hasher(
		max_size: CacheSize,
		policies: &[PaperPolicy],
		policy: PaperPolicy,
		hasher: S,
	) -> Result<Self, CacheError> {
		if max_size == 0 {
			return Err(CacheError::ZeroCacheSize);
		}

		if policies.is_empty() {
			return Err(CacheError::EmptyPolicies);
		}

		if policies.contains(&PaperPolicy::Auto) {
			return Err(CacheError::ConfiguredAutoPolicy);
		}

		if policies.iter().is_multiset() {
			return Err(CacheError::DuplicatePolicies);
		}

		if !policy.is_auto() && !policies.contains(&policy) {
			return Err(CacheError::UnconfiguredPolicy);
		}

		let objects = Arc::new(DashMap::with_hasher(NoHasher::default()));
		let status = Arc::new(AtomicStatus::new(max_size, policies, policy)?);
		let overhead_manager = Arc::new(OverheadManager::new(&status));

		let (worker_sender, worker_listener) = unbounded();

		let mut worker_manager = WorkerManager::new(
			worker_listener,
			&objects,
			&status,
			&overhead_manager,
		)?;

		thread::spawn(move || worker_manager.run());

		let cache = PaperCache {
			objects,
			status,

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
	/// let mut cache = PaperCache::<u32, u32>::new(
	///     1000,
	///     &[PaperPolicy::Lfu],
	///     PaperPolicy::Lfu
	/// ).unwrap();
	///
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
	/// let mut cache = PaperCache::<u32, u32>::new(
	///     1000,
	///     &[PaperPolicy::Lfu],
	///     PaperPolicy::Lfu,
	/// ).unwrap();
	///
	/// cache.set(0, 0, None);
	///
	/// let status = cache.status().unwrap();
	/// assert!(status.used_size() > 0);
	/// ```
	pub fn status(&self) -> Result<Status, CacheError> {
		self.status.try_to_status()
	}

	/// Gets the value associated with the supplied key.
	/// If the key was not found in the cache, returns a [`CacheError`].
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, PaperPolicy};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(
	///     1000,
	///     &[PaperPolicy::Lfu],
	///     PaperPolicy::Lfu,
	/// ).unwrap();
	///
	/// cache.set(0, 0, None);
	///
	/// // Getting a key which exists in the cache will return the associated value.
	/// assert!(cache.get(&0).is_ok());
	/// // Getting a key which does not exist in the cache will return a CacheError.
	/// assert!(cache.get(&1).is_err());
	/// ```
	pub fn get(&self, key: &K) -> Result<Arc<V>, CacheError> {
		let hashed_key = self.hash_key(key);

		let result = match self.objects.get(&hashed_key) {
			Some(object) if object.key_matches(key) && !object.is_expired() => {
				self.status.incr_hits();
				Ok(object.data())
			},

			_ => {
				self.status.incr_misses();
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
	/// let mut cache = PaperCache::<u32, u32>::new(
	///     1000,
	///     &[PaperPolicy::Lfu],
	///     PaperPolicy::Lfu,
	/// ).unwrap();
	///
	/// assert!(cache.set(0, 0, None).is_ok());
	/// ```
	pub fn set(&self, key: K, value: V, ttl: Option<u32>) -> Result<(), CacheError> {
		let hashed_key = self.hash_key(&key);

		let object = Object::new(key, value, ttl);
		let base_size = self.overhead_manager.base_size(&object);
		let expiry = object.expiry();

		if base_size == 0 {
			return Err(CacheError::ZeroValueSize);
		}

		if self.status.exceeds_max_size(base_size) {
			return Err(CacheError::ExceedingValueSize);
		}

		self.status.incr_sets();

		let old_object_info = self.objects
			.insert(hashed_key, object)
			.map(|old_object| {
				let base_size = self.overhead_manager.base_size(&old_object);
				let expiry = old_object.expiry();

				(base_size, expiry)
			});

		let base_size_delta = if let Some((old_object_size, _)) = old_object_info {
			base_size as i64 - old_object_size as i64
		} else {
			// the object is new, so increase the number of objects count
			self.status.incr_num_objects();
			base_size as i64
		};

		self.status.update_base_used_size(base_size_delta);
		self.broadcast(WorkerEvent::Set(hashed_key, base_size, expiry, old_object_info))?;

		Ok(())
	}

	/// Deletes the object associated with the supplied key in the cache.
	/// Returns a [`CacheError`] if the key was not found in the cache.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, PaperPolicy};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(
	///     1000,
	///     &[PaperPolicy::Lfu],
	///     PaperPolicy::Lfu,
	/// ).unwrap();
	///
	/// cache.set(0, 0, None);
	/// assert!(cache.del(&0).is_ok());
	///
	/// // Deleting a key which does not exist in the cache will return a CacheError.
	/// assert!(cache.del(&1).is_err());
	/// ```
	pub fn del(&self, key: &K) -> Result<(), CacheError> {
		let hashed_key = self.hash_key(key);

		let (removed_hashed_key, object) = erase(
			&self.objects,
			&self.status,
			&self.overhead_manager,
			Some(EraseKey::Original(key, hashed_key)),
		)?;

		self.status.incr_dels();
		self.broadcast(WorkerEvent::Del(removed_hashed_key, object.expiry()))?;

		Ok(())
	}

	/// Checks if an object with the supplied key exists in the cache without
	/// altering any of the cache's internal queues.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, PaperPolicy};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(
	///     1000,
	///     &[PaperPolicy::Lfu],
	///     PaperPolicy::Lfu,
	/// ).unwrap();
	///
	/// cache.set(0, 0, None);
	///
	/// assert!(cache.has(&0));
	/// assert!(!cache.has(&1));
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
	/// let mut cache = PaperCache::<u32, u32>::new(
	///     1000,
	///     &[PaperPolicy::Lfu],
	///     PaperPolicy::Lfu,
	/// ).unwrap();
	///
	/// cache.set(0, 0, None);
	/// cache.set(1, 0, None);
	///
	/// // Peeking a key which exists in the cache will return the associated value.
	/// assert!(cache.peek(&0).is_ok());
	/// // Peeking a key which does not exist in the cache will return a CacheError.
	/// assert!(cache.peek(&2).is_err());
	///
	/// cache.set(2, 0, None);
	///
	/// // Peeking a key will not alter the eviction order of the objects.
	/// assert!(cache.peek(&1).is_ok());
	/// assert!(cache.peek(&2).is_ok());
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
	/// let mut cache = PaperCache::<u32, u32>::new(
	///     1000,
	///     &[PaperPolicy::Lfu],
	///     PaperPolicy::Lfu,
	/// ).unwrap();
	///
	/// cache.set(0, 0, None); // value will not expire
	/// cache.ttl(&0, Some(5)); // value will expire in 5 seconds
	/// ```
	pub fn ttl(&self, key: &K, ttl: Option<u32>) -> Result<(), CacheError> {
		let hashed_key = self.hash_key(key);

		let mut object = match self.objects.get_mut(&hashed_key) {
			Some(object) if object.key_matches(key) && !object.is_expired() => object,
			_ => return Err(CacheError::KeyNotFound),
		};

		let old_expiry = object.expiry();
		let old_base_size = self.overhead_manager.base_size(&object);

		object.expires(ttl);

		let new_expiry = object.expiry();
		let new_base_size = self.overhead_manager.base_size(&object);

		self.status.update_base_used_size(new_base_size as i64 - old_base_size as i64);
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
	/// let mut cache = PaperCache::<u32, u32>::new(
	///     1000,
	///     &[PaperPolicy::Lfu],
	///     PaperPolicy::Lfu,
	/// ).unwrap();
	///
	/// cache.set(0, 0, None);
	///
	/// // Sizing a key which exists in the cache will return the size of the associated value.
	/// assert!(cache.size(&0).is_ok());
	/// // Sizing a key which does not exist in the cache will return a CacheError.
	/// assert!(cache.size(&1).is_err());
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
	/// let mut cache = PaperCache::<u32, u32>::new(
	///     1000,
	///     &[PaperPolicy::Lfu],
	///     PaperPolicy::Lfu,
	/// ).unwrap();
	///
	/// cache.wipe();
	/// ```
	pub fn wipe(&self) -> Result<(), CacheError> {
		info!("Wiping cache");

		self.objects.clear();
		self.status.clear();

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
	/// let mut cache = PaperCache::<u32, u32>::new(
	///     1000,
	///     &[PaperPolicy::Lfu],
	///     PaperPolicy::Lfu,
	/// ).unwrap();
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

		let current_max_size = self.status.max_size();

		if max_size == current_max_size {
			return Ok(());
		}

		info!(
			"Resizing cache from {} to {}",
			fmt::memory(current_max_size, Some(2)),
			fmt::memory(max_size, Some(2)),
		);

		self.status.set_max_size(max_size);
		self.broadcast(WorkerEvent::Resize(max_size))?;

		Ok(())
	}

	/// Sets the eviction policy of the cache to the supplied policy.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{PaperCache, PaperPolicy};
	///
	/// let mut cache = PaperCache::<u32, u32>::new(
	///     1000,
	///     &[PaperPolicy::Lfu],
	///     PaperPolicy::Lfu,
	/// ).unwrap();
	///
	/// assert!(cache.policy(PaperPolicy::Lfu).is_ok());
	/// assert!(cache.policy(PaperPolicy::Lru).is_err());
	/// ```
	pub fn policy(&self, policy: PaperPolicy) -> Result<(), CacheError> {
		if !policy.is_auto() && !self.status.policies().contains(&policy) {
			return Err(CacheError::UnconfiguredPolicy);
		}

		self.status.set_policy(policy)?;
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
	status: &StatusRef,
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

	if let Some(EraseKey::Original(key, _)) = maybe_key && !entry.get().key_matches(key) {
		return Err(CacheError::KeyNotFound);
	};

	let object = entry.remove();
	let base_size = overhead_manager.base_size(&object) as i64;

	status.update_base_used_size(-base_size);
	status.decr_num_objects();

	match !object.is_expired() {
		true => Ok((hashed_key, object)),
		false => Err(CacheError::KeyNotFound),
	}
}

unsafe impl<K, V, S> Send for PaperCache<K, V, S> {}

#[cfg(test)]
mod tests {
	use crate::{PaperCache, PaperPolicy, CacheError};

	const TEST_CACHE_MAX_SIZE: u64 = 1000;

	#[test]
	fn it_returns_correct_version() {
		let cache = init_test_cache();
		assert_eq!(cache.version(), env!("CARGO_PKG_VERSION"));
	}

	#[test]
	fn it_returns_status() {
		let cache = init_test_cache();
		let status = cache.status().unwrap();

		assert_eq!(status.max_size(), TEST_CACHE_MAX_SIZE);
	}

	#[test]
	fn it_gets_an_existing_object() {
		let cache = init_test_cache();

		assert!(cache.set(0, 1, None).is_ok());
		assert_eq!(cache.get(&0).as_deref(), Ok(&1));
	}

	#[test]
	fn it_does_not_get_a_non_existing_object() {
		let cache = init_test_cache();

		assert!(cache.set(0, 1, None).is_ok());
		assert_eq!(cache.get(&1), Err(CacheError::KeyNotFound));
	}

	#[test]
	fn it_calculates_miss_ratio_correctly() {
		let cache = init_test_cache();

		assert!(cache.set(0, 1, None).is_ok());
		assert!(cache.get(&0).is_ok());
		assert!(cache.get(&0).is_ok());
		assert!(cache.get(&0).is_ok());
		assert!(cache.get(&1).is_err());

		let status = cache.status().unwrap();
		assert_eq!(status.miss_ratio(), 0.25);
	}

	#[test]
	fn it_sets_with_no_ttl() {
		let cache = init_test_cache();

		assert!(cache.set(0, 1, None).is_ok());
		assert!(cache.get(&0).is_ok());
	}

	#[test]
	fn it_sets_with_ttl() {
		use std::{
			thread,
			time::Duration,
		};

		let cache = init_test_cache();
		assert!(cache.set(0, 1, Some(1)).is_ok());

		assert!(cache.get(&0).is_ok());
		thread::sleep(Duration::from_secs(2));
		assert!(cache.get(&0).is_err());
	}

	#[test]
	fn it_dels_an_existing_object() {
		let cache = init_test_cache();
		assert!(cache.set(0, 1, Some(1)).is_ok());

		assert!(cache.get(&0).is_ok());
		assert!(cache.del(&0).is_ok());
		assert!(cache.get(&0).is_err());
	}

	#[test]
	fn it_does_not_del_a_non_existing_object() {
		let cache = init_test_cache();
		assert_eq!(cache.del(&0), Err(CacheError::KeyNotFound));
	}

	#[test]
	fn it_does_not_allow_empty_policies() {
		let try_cache = PaperCache::<u32, u32>::new(
			TEST_CACHE_MAX_SIZE,
			&[],
			PaperPolicy::Lfu,
		);

		assert!(try_cache.is_err_and(|err| err == CacheError::EmptyPolicies));

		let try_cache = PaperCache::<u32, u32>::new(
			TEST_CACHE_MAX_SIZE,
			&[],
			PaperPolicy::Auto,
		);

		assert!(try_cache.is_err_and(|err| err == CacheError::EmptyPolicies));
	}

	#[test]
	fn it_does_not_allow_auto_policy_in_configured_policies() {
		let try_cache = PaperCache::<u32, u32>::new(
			TEST_CACHE_MAX_SIZE,
			&[PaperPolicy::Auto],
			PaperPolicy::Auto,
		);

		assert!(try_cache.is_err_and(|err| err == CacheError::ConfiguredAutoPolicy));

		let try_cache = PaperCache::<u32, u32>::new(
			TEST_CACHE_MAX_SIZE,
			&[PaperPolicy::Auto, PaperPolicy::Lru],
			PaperPolicy::Auto,
		);

		assert!(try_cache.is_err_and(|err| err == CacheError::ConfiguredAutoPolicy));
	}

	#[test]
	fn it_does_not_allow_duplicate_policies() {
		let try_cache = PaperCache::<u32, u32>::new(
			TEST_CACHE_MAX_SIZE,
			&[PaperPolicy::Lfu, PaperPolicy::Lru, PaperPolicy::Lfu],
			PaperPolicy::Lfu,
		);

		assert!(try_cache.is_err_and(|err| err == CacheError::DuplicatePolicies));

		let try_cache = PaperCache::<u32, u32>::new(
			TEST_CACHE_MAX_SIZE,
			&[PaperPolicy::Lfu, PaperPolicy::Lru],
			PaperPolicy::Lfu,
		);

		assert!(try_cache.is_ok());
	}

	#[test]
	fn it_has_an_existing_object() {
		let cache = init_test_cache();

		assert!(cache.set(0, 1, Some(1)).is_ok());
		assert!(cache.has(&0));
	}

	#[test]
	fn it_does_not_have_a_non_existing_object() {
		let cache = init_test_cache();
		assert!(!cache.has(&1));
	}

	#[test]
	fn it_peeks_an_existing_object() {
		let cache = init_test_cache();

		assert!(cache.set(0, 1, None).is_ok());
		assert_eq!(cache.peek(&0).as_deref(), Ok(&1));
	}

	#[test]
	fn it_does_not_peek_a_non_existing_object() {
		let cache = init_test_cache();

		assert!(cache.set(0, 1, None).is_ok());
		assert_eq!(cache.peek(&1), Err(CacheError::KeyNotFound));
	}

	#[test]
	fn it_sets_an_existing_objects_ttl() {
		use std::{
			thread,
			time::Duration,
		};

		let cache = init_test_cache();

		assert!(cache.set(0, 1, None).is_ok());
		assert!(cache.get(&0).is_ok());

		assert!(cache.ttl(&0, Some(1)).is_ok());

		thread::sleep(Duration::from_secs(2));
		assert_eq!(cache.get(&0), Err(CacheError::KeyNotFound));
	}

	#[test]
	fn it_does_not_set_a_non_existing_objects_ttl() {
		let cache = init_test_cache();
		assert_eq!(cache.ttl(&0, Some(1)), Err(CacheError::KeyNotFound));
	}

	#[test]
	fn it_resets_an_objects_ttl() {
		use std::{
			thread,
			time::Duration,
		};

		let cache = init_test_cache();

		assert!(cache.set(0, 1, Some(1)).is_ok());
		assert!(cache.get(&0).is_ok());

		assert!(cache.ttl(&0, Some(5)).is_ok());

		thread::sleep(Duration::from_secs(2));
		assert!(cache.get(&0).is_ok());
	}

	#[test]
	fn it_gets_an_objects_size() {
		use std::mem;

		use crate::object::{
			ExpireTime,
			overhead::get_policy_overhead,
		};

		let cache = init_test_cache();

		let expected = 4 + 4
			+ mem::size_of::<ExpireTime>() as u32
			+ get_policy_overhead(&PaperPolicy::Lfu);

		assert!(cache.set(0, 1, None).is_ok());
		assert_eq!(cache.size(&0), Ok(expected));
	}

	#[test]
	fn it_gets_an_expiring_objects_size() {
		use std::mem;

		use crate::object::{
			ExpireTime,
			overhead::{get_policy_overhead, get_ttl_overhead},
		};

		let cache = init_test_cache();

		let expected = 4 + 4
			+ mem::size_of::<ExpireTime>() as u32
			+ get_policy_overhead(&PaperPolicy::Lfu)
			+ get_ttl_overhead();

		assert!(cache.set(0, 1, Some(1)).is_ok());
		assert_eq!(cache.size(&0), Ok(expected));
	}

	#[test]
	fn it_gets_an_objects_size_after_policy_switch() {
		use std::mem;

		use crate::object::{
			ExpireTime,
			overhead::get_policy_overhead,
		};

		let cache = PaperCache::<u32, u32>::new(
			TEST_CACHE_MAX_SIZE,
			&[PaperPolicy::Lru, PaperPolicy::Lfu],
			PaperPolicy::Lfu,
		).expect("Could not initialize test cache.");

		let base_expected = 4 + 4 + mem::size_of::<ExpireTime>() as u32;
		let lfu_expected = base_expected + get_policy_overhead(&PaperPolicy::Lfu);
		let lru_expected = base_expected + get_policy_overhead(&PaperPolicy::Lru);

		assert!(cache.set(0, 1, None).is_ok());
		assert_eq!(cache.size(&0), Ok(lfu_expected));

		assert!(cache.policy(PaperPolicy::Lru).is_ok());
		assert_eq!(cache.size(&0), Ok(lru_expected));
	}

	#[test]
	fn it_gets_an_objects_size_after_set_ttl() {
		use std::mem;

		use crate::object::{
			ExpireTime,
			overhead::{get_policy_overhead, get_ttl_overhead},
		};

		let cache = init_test_cache();

		let pre_expected = 4 + 4
			+ mem::size_of::<ExpireTime>() as u32
			+ get_policy_overhead(&PaperPolicy::Lfu);

		let post_expected = pre_expected + get_ttl_overhead();

		assert!(cache.set(0, 1, None).is_ok());
		assert_eq!(cache.size(&0), Ok(pre_expected));

		assert!(cache.ttl(&0, Some(1)).is_ok());
		assert_eq!(cache.size(&0), Ok(post_expected));
	}

	#[test]
	fn it_gets_an_objects_size_after_unset_ttl() {
		use std::mem;

		use crate::object::{
			ExpireTime,
			overhead::{get_policy_overhead, get_ttl_overhead},
		};

		let cache = init_test_cache();

		let pre_expected = 4 + 4
			+ mem::size_of::<ExpireTime>() as u32
			+ get_policy_overhead(&PaperPolicy::Lfu)
			+ get_ttl_overhead();

		let post_expected = pre_expected - get_ttl_overhead();

		assert!(cache.set(0, 1, Some(1)).is_ok());
		assert_eq!(cache.size(&0), Ok(pre_expected));

		assert!(cache.ttl(&0, None).is_ok());
		assert_eq!(cache.size(&0), Ok(post_expected));
	}

	#[test]
	fn status_shows_correct_used_size() {
		use std::mem;

		use crate::object::{
			ExpireTime,
			overhead::{get_policy_overhead, get_ttl_overhead},
		};

		let cache = init_test_cache();

		let expected = (4 + 4) * 2
			+ mem::size_of::<ExpireTime>() as u32 * 2
			+ get_policy_overhead(&PaperPolicy::Lfu) * 2
			+ get_ttl_overhead();

		assert!(cache.set(0, 1, None).is_ok());
		assert!(cache.set(1, 1, Some(1)).is_ok());

		let status = cache.status().unwrap();
		assert_eq!(status.used_size(), expected as u64);
	}

	#[test]
	fn status_shows_correct_used_size_after_policy_switch() {
		use std::mem;

		use crate::object::{
			ExpireTime,
			overhead::get_policy_overhead,
		};

		let cache = PaperCache::<u32, u32>::new(
			TEST_CACHE_MAX_SIZE,
			&[PaperPolicy::Lru, PaperPolicy::Lfu],
			PaperPolicy::Lfu,
		).expect("Could not initialize test cache.");

		let base_expected = 4 + 4 + mem::size_of::<ExpireTime>() as u32;
		let lfu_expected = base_expected + get_policy_overhead(&PaperPolicy::Lfu);
		let lru_expected = base_expected + get_policy_overhead(&PaperPolicy::Lru);

		assert!(cache.set(0, 1, None).is_ok());
		let status = cache.status().unwrap();
		assert_eq!(status.used_size(), lfu_expected as u64);

		assert!(cache.policy(PaperPolicy::Lru).is_ok());
		let status = cache.status().unwrap();
		assert_eq!(status.used_size(), lru_expected as u64);
	}

	#[test]
	fn status_shows_correct_used_size_after_set_ttl() {
		use std::mem;

		use crate::object::{
			ExpireTime,
			overhead::{get_policy_overhead, get_ttl_overhead},
		};

		let cache = init_test_cache();

		let pre_expected = 4 + 4
			+ mem::size_of::<ExpireTime>() as u32
			+ get_policy_overhead(&PaperPolicy::Lfu);

		let post_expected = pre_expected + get_ttl_overhead();

		assert!(cache.set(0, 1, None).is_ok());
		let status = cache.status().unwrap();
		assert_eq!(status.used_size(), pre_expected as u64);

		assert!(cache.ttl(&0, Some(1)).is_ok());
		let status = cache.status().unwrap();
		assert_eq!(status.used_size(), post_expected as u64);
	}

	#[test]
	fn status_shows_correct_used_size_after_unset_ttl() {
		use std::mem;

		use crate::object::{
			ExpireTime,
			overhead::{get_policy_overhead, get_ttl_overhead},
		};

		let cache = init_test_cache();

		let pre_expected = 4 + 4
			+ mem::size_of::<ExpireTime>() as u32
			+ get_policy_overhead(&PaperPolicy::Lfu)
			+ get_ttl_overhead();

		let post_expected = pre_expected - get_ttl_overhead();

		assert!(cache.set(0, 1, Some(1)).is_ok());
		let status = cache.status().unwrap();
		assert_eq!(status.used_size(), pre_expected as u64);

		assert!(cache.ttl(&0, None).is_ok());
		let status = cache.status().unwrap();
		assert_eq!(status.used_size(), post_expected as u64);
	}

	fn init_test_cache() -> PaperCache<u32, u32> {
		PaperCache::<u32, u32>::new(
			TEST_CACHE_MAX_SIZE,
			&[PaperPolicy::Lfu],
			PaperPolicy::Lfu,
		).expect("Could not initialize test cache.")
	}
}
