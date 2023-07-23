use kwik::utils;

use crate::{
    cache::CacheSize,
    policy::Policy,
};

#[derive(Clone, Copy)]
pub struct Stats {
	max_size: CacheSize,
	used_size: CacheSize,

	total_hits: u64,
	total_gets: u64,
	total_sets: u64,
	total_dels: u64,

	policy: Policy,

	start_time: u64,
}

/// This struct holds the basic statistical information about `PaperCache`.
impl Stats {
	/// Creates an empty statistics manager.
	pub fn new(max_size: CacheSize, policy: Policy) -> Self {
		Stats {
			max_size,
			used_size: 0,

			total_hits: 0,
			total_gets: 0,
			total_sets: 0,
			total_dels: 0,

			policy,

			start_time: utils::timestamp(),
		}
	}

	/// Returns the cache's maximum size.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{Stats, Policy};
	///
	/// let stats = Stats::new(10, Policy::Lru);
	/// assert_eq!(stats.get_max_size(), 10);
	/// ```
	pub fn get_max_size(&self) -> CacheSize {
		self.max_size
	}

	/// Returns the cache's used size.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{Stats, Policy};
	///
	/// let mut stats = Stats::new(10, Policy::Lru);
	///
	/// // The cache is currently empty.
	/// assert_eq!(stats.get_used_size(), 0);
	///
	/// // The cache gets filled.
	/// stats.increase_used_size(10);
	/// assert_eq!(stats.get_used_size(), 10);
	/// ```
	pub fn get_used_size(&self) -> CacheSize {
		self.used_size
	}

	/// Returns the cache's total number of gets.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{Stats, Policy};
	///
	/// let mut stats = Stats::new(10, Policy::Lru);
	///
	/// assert_eq!(stats.get_total_gets(), 0);
	///
	/// stats.hit();
	///
	/// assert_eq!(stats.get_total_gets(), 1);
	/// ```
	pub fn get_total_gets(&self) -> u64 {
		self.total_gets
	}

	/// Returns the cache's total number of sets.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{Stats, Policy};
	///
	/// let mut stats = Stats::new(10, Policy::Lru);
	///
	/// assert_eq!(stats.get_total_sets(), 0);
	///
	/// stats.set();
	///
	/// assert_eq!(stats.get_total_sets(), 1);
	/// ```
	pub fn get_total_sets(&self) -> u64 {
		self.total_sets
	}

	/// Returns the cache's total number of dels.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{Stats, Policy};
	///
	/// let mut stats = Stats::new(10, Policy::Lru);
	///
	/// assert_eq!(stats.get_total_dels(), 0);
	///
	/// stats.del();
	///
	/// assert_eq!(stats.get_total_dels(), 1);
	/// ```
	pub fn get_total_dels(&self) -> u64 {
		self.total_dels
	}

	/// Returns the cache's current miss ratio.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{Stats, Policy};
	///
	/// let mut stats = Stats::new(10, Policy::Lru);
	///
	/// // The cache has not had any gets yet, therefore the
	/// // miss ratio should be one.
	/// assert_eq!(stats.get_miss_ratio(), 1.0);
	///
	/// // The cache gets hits.
	/// stats.hit();
	/// stats.hit();
	/// stats.hit();
	/// stats.miss();
	///
	/// assert_eq!(stats.get_miss_ratio(), 0.25);
	/// ```
	pub fn get_miss_ratio(&self) -> f64 {
		if self.total_gets == 0 {
			return 1.0;
		}

		1.0 - self.total_hits as f64 / self.total_gets as f64
	}

	/// Returns the cache's current eviction policy.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{Stats, Policy};
	///
	/// let mut stats = Stats::new(10, Policy::Lru);
	///
	/// assert_eq!(stats.get_policy(), &Policy::Lru);
	/// ```
	pub fn get_policy(&self) -> &Policy {
		&self.policy
	}

	/// Returns the cache's current uptime.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{Stats, Policy};
	///
	/// let mut stats = Stats::new(10, Policy::Lru);
	///
	/// assert!(stats.get_uptime() >= 0);
	/// ```
	pub fn get_uptime(&self) -> u64 {
		utils::timestamp() - self.start_time
	}

	/// Records a cache hit and increments the total gets counter.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{Stats, Policy};
	///
	/// let mut stats = Stats::new(10, Policy::Lru);
	///
	/// assert_eq!(stats.get_total_gets(), 0);
	/// assert_eq!(stats.get_miss_ratio(), 1.0);
	///
	/// stats.hit();
	///
	/// assert_eq!(stats.get_total_gets(), 1);
	/// assert_eq!(stats.get_miss_ratio(), 0.0);
	/// ```
	pub fn hit(&mut self) {
		self.total_gets += 1;
		self.total_hits += 1;
	}

	/// Records a cache miss and increments the total gets counter.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{Stats, Policy};
	///
	/// let mut stats = Stats::new(10, Policy::Lru);
	///
	/// assert_eq!(stats.get_total_gets(), 0);
	/// assert_eq!(stats.get_miss_ratio(), 1.0);
	///
	/// stats.miss();
	///
	/// assert_eq!(stats.get_total_gets(), 1);
	/// assert_eq!(stats.get_miss_ratio(), 1.0);
	/// ```
	pub fn miss(&mut self) {
		self.total_gets += 1;
	}

	/// Increments the total sets counter.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{Stats, Policy};
	///
	/// let mut stats = Stats::new(10, Policy::Lru);
	///
	/// assert_eq!(stats.get_total_sets(), 0);
	///
	/// stats.set();
	///
	/// assert_eq!(stats.get_total_sets(), 1);
	/// ```
	pub fn set(&mut self) {
		self.total_sets += 1;
	}

	/// Increments the total dels counter.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{Stats, Policy};
	///
	/// let mut stats = Stats::new(10, Policy::Lru);
	///
	/// assert_eq!(stats.get_total_dels(), 0);
	///
	/// stats.del();
	///
	/// assert_eq!(stats.get_total_dels(), 1);
	/// ```
	pub fn del(&mut self) {
		self.total_dels += 1;
	}

	/// Sets the cache's maximum size.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{Stats, Policy};
	///
	/// let mut stats = Stats::new(10, Policy::Lru);
	/// assert_eq!(stats.get_max_size(), 10);
	///
	/// stats.set_max_size(5);
	/// assert_eq!(stats.get_max_size(), 5);
	/// ```
	pub fn set_max_size(&mut self, max_size: u64) {
		self.max_size = max_size;
	}

	/// Increases the cache's used size by the supplied amount.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{Stats, Policy};
	///
	/// let mut stats = Stats::new(10, Policy::Lru);
	///
	/// assert_eq!(stats.get_used_size(), 0);
	///
	/// stats.increase_used_size(5);
	/// assert_eq!(stats.get_used_size(), 5);
	/// ```
	pub fn increase_used_size(&mut self, size: u64) {
		self.used_size += size;
	}

	/// Decreases the cache's used size by the supplied amount.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{Stats, Policy};
	///
	/// let mut stats = Stats::new(10, Policy::Lru);
	///
	/// stats.increase_used_size(10);
	/// assert_eq!(stats.get_used_size(), 10);
	///
	/// stats.decrease_used_size(5);
	/// assert_eq!(stats.get_used_size(), 5);
	/// ```
	pub fn decrease_used_size(&mut self, size: u64) {
		self.used_size -= size;
	}

	/// Sets the cache's used size to zero.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{Stats, Policy};
	///
	/// let mut stats = Stats::new(10, Policy::Lru);
	///
	/// stats.increase_used_size(5);
	/// assert_eq!(stats.get_used_size(), 5);
	///
	/// stats.reset_used_size();
	/// assert_eq!(stats.get_used_size(), 0);
	/// ```
	pub fn reset_used_size(&mut self) {
		self.used_size = 0;
	}

	/// Sets the cache's policy.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{Stats, Policy};
	///
	/// let mut stats = Stats::new(10, Policy::Lru);
	///
	/// stats.set_policy(Policy::Mru);
	/// assert_eq!(stats.get_policy(), &Policy::Mru);
	/// ```
	pub fn set_policy(&mut self, policy: Policy) {
		self.policy = policy;
	}

	/// Returns true if the supplied size exceeds the cache's maximum size.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{Stats, Policy};
	///
	/// let mut stats = Stats::new(10, Policy::Lru);
	/// assert!(stats.exceeds_max_size(15));
	/// assert!(!stats.exceeds_max_size(5));
	/// ```
	pub fn exceeds_max_size(&self, size: u64) -> bool {
		size > self.max_size
	}

	/// Returns true if the cache's used size exceeds the supplied size.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{Stats, Policy};
	///
	/// let mut stats = Stats::new(10, Policy::Lru);
	///
	/// stats.increase_used_size(10);
	///
	/// assert!(stats.used_size_exceeds(5));
	/// assert!(!stats.used_size_exceeds(15));
	/// ```
	pub fn used_size_exceeds(&self, size: u64) -> bool {
		self.used_size > size
	}

	/// Returns the target used size of the cache to be able to fit the
	/// supplied size without exceeding the maximum size.
	///
	/// # Examples
	/// ```
	/// use paper_cache::{Stats, Policy};
	///
	/// let mut stats = Stats::new(10, Policy::Lru);
	///
	/// assert_eq!(stats.target_used_size_to_fit(2), 8);
	/// ```
	pub fn target_used_size_to_fit(&self, size: u64) -> u64 {
		self.max_size - size
	}
}
