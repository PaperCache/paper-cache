use kwik::utils;

use std::sync::{
	Arc,
	atomic::{Ordering, AtomicU64, AtomicUsize},
};

use crate::{
	Policy,
	paper_cache::{CacheSize, AtomicCacheSize},
};

pub struct AtomicStats {
	max_size: AtomicCacheSize,
	used_size: AtomicCacheSize,

	total_hits: AtomicU64,
	total_gets: AtomicU64,
	total_sets: AtomicU64,
	total_dels: AtomicU64,

	policy_index: AtomicUsize,

	start_time: AtomicU64,
}

#[derive(Debug)]
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
	/// Returns the cache's maximum size.
	#[must_use]
	pub fn get_max_size(&self) -> CacheSize {
		self.max_size
	}

	/// Returns the cache's used size.
	#[must_use]
	pub fn get_used_size(&self) -> CacheSize {
		self.used_size
	}

	/// Returns the cache's total number of gets.
	#[must_use]
	pub fn get_total_gets(&self) -> u64 {
		self.total_gets
	}

	/// Returns the cache's total number of sets.
	#[must_use]
	pub fn get_total_sets(&self) -> u64 {
		self.total_sets
	}

	/// Returns the cache's total number of dels.
	#[must_use]
	pub fn get_total_dels(&self) -> u64 {
		self.total_dels
	}

	/// Returns the cache's current miss ratio.
	#[must_use]
	pub fn get_miss_ratio(&self) -> f64 {
		if self.total_gets == 0 {
			return 1.0;
		}

		1.0 - self.total_hits as f64 / self.total_gets as f64
	}

	/// Returns the cache's current eviction policy index.
	#[must_use]
	pub fn get_policy(&self) -> Policy {
		self.policy
	}

	/// Returns the cache's current uptime.
	#[must_use]
	pub fn get_uptime(&self) -> u64 {
		utils::timestamp() - self.start_time
	}
}

/// This struct holds the basic statistical information about `PaperCache`
/// and allows for atomic updates of its fields.
impl AtomicStats {
	#[must_use]
	pub fn new(max_size: CacheSize, policy_index: usize) -> Self {
		AtomicStats {
			max_size: AtomicU64::new(max_size),
			used_size: AtomicU64::default(),

			total_hits: AtomicU64::default(),
			total_gets: AtomicU64::default(),
			total_sets: AtomicU64::default(),
			total_dels: AtomicU64::default(),

			policy_index: AtomicUsize::new(policy_index),

			start_time: AtomicU64::new(utils::timestamp()),
		}
	}

	#[must_use]
	pub fn get_max_size(&self) -> CacheSize {
		self.max_size.load(Ordering::Relaxed)
	}

	pub fn hit(&self) {
		self.total_gets.fetch_add(1, Ordering::Relaxed);
		self.total_hits.fetch_add(1, Ordering::Relaxed);
	}

	pub fn miss(&self) {
		self.total_gets.fetch_add(1, Ordering::Relaxed);
	}

	pub fn set(&self) {
		self.total_sets.fetch_add(1, Ordering::Relaxed);
	}

	pub fn del(&self) {
		self.total_dels.fetch_add(1, Ordering::Relaxed);
	}

	pub fn set_max_size(&self, max_size: u64) {
		self.max_size.store(max_size, Ordering::Relaxed);
	}

	pub fn increase_used_size(&self, size: u64) {
		self.used_size.fetch_add(size, Ordering::Relaxed);
	}

	pub fn decrease_used_size(&self, size: u64) {
		self.used_size.fetch_sub(size, Ordering::Relaxed);
	}

	pub fn reset_used_size(&self) {
		self.used_size.store(0, Ordering::Relaxed);
	}

	pub fn set_policy_index(&self, policy_index: usize) {
		self.policy_index.store(policy_index, Ordering::Relaxed);
	}

	#[must_use]
	pub fn exceeds_max_size(&self, size: u64) -> bool {
		size > self.max_size.load(Ordering::Relaxed)
	}

	#[must_use]
	pub fn used_size_exceeds(&self, size: u64) -> bool {
		self.used_size.load(Ordering::Relaxed) > size
	}

	#[must_use]
	pub fn target_used_size_to_fit(&self, size: u64) -> u64 {
		self.max_size.load(Ordering::Relaxed) - size
	}

	#[must_use]
	pub fn to_stats(&self, policies: Arc<Box<[Policy]>>) -> Stats {
		Stats {
			max_size: self.get_max_size(),
			used_size: self.used_size.load(Ordering::Relaxed),

			total_hits: self.total_hits.load(Ordering::Relaxed),
			total_gets: self.total_gets.load(Ordering::Relaxed),
			total_sets: self.total_sets.load(Ordering::Relaxed),
			total_dels: self.total_dels.load(Ordering::Relaxed),

			policy: policies[self.policy_index.load(Ordering::Relaxed)],

			start_time: self.start_time.load(Ordering::Relaxed),
		}
	}
}
