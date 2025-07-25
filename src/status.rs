/*
 * Copyright (c) Kia Shakiba
 *
 * This source code is licensed under the GNU AGPLv3 license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::{
	process,
	sync::{
		Arc,
		atomic::{Ordering, AtomicBool, AtomicU64, AtomicUsize},
	},
};

use num_traits::AsPrimitive;
use log::error;

use kwik::{
	time,
	sys::mem,
};

use crate::{
	CacheSize,
	AtomicCacheSize,
	error::CacheError,
	policy::PaperPolicy,
	object::overhead::get_policy_overhead,
};

#[derive(Debug)]
pub struct Status {
	pid: u32,

	max_size: CacheSize,
	used_size: CacheSize,
	num_objects: u64,

	rss: u64,
	hwm: u64,

	total_hits: u64,
	total_gets: u64,
	total_sets: u64,
	total_dels: u64,

	policies: Arc<[PaperPolicy]>,
	policy: PaperPolicy,
	is_auto_policy: bool,

	start_time: u64,
}

pub struct AtomicStatus {
	max_size: AtomicCacheSize,
	base_used_size: AtomicCacheSize,
	num_objects: AtomicU64,

	total_hits: AtomicU64,
	total_gets: AtomicU64,
	total_sets: AtomicU64,
	total_dels: AtomicU64,

	policies: Arc<[PaperPolicy]>,
	policy_index: AtomicUsize,
	is_auto_policy: AtomicBool,

	start_time: AtomicU64,
}

/// This struct holds the basic statistical information about `PaperCache`.
impl Status {
	/// Returns the cache's PID.
	#[must_use]
	pub fn pid(&self) -> u32 {
		self.pid
	}

	/// Returns the cache's maximum size.
	#[must_use]
	pub fn max_size(&self) -> CacheSize {
		self.max_size
	}

	/// Returns the cache's used size.
	#[must_use]
	pub fn used_size(&self) -> CacheSize {
		self.used_size
	}

	/// Returns the number of objects in the cache.
	#[must_use]
	pub fn num_objects(&self) -> u64 {
		self.num_objects
	}

	/// Returns the cache's resident set size.
	#[must_use]
	pub fn rss(&self) -> u64 {
		self.rss
	}

	/// Returns the cache's resident set size high water mark.
	#[must_use]
	pub fn hwm(&self) -> u64 {
		self.hwm
	}

	/// Returns the cache's total number of gets.
	#[must_use]
	pub fn total_gets(&self) -> u64 {
		self.total_gets
	}

	/// Returns the cache's total number of sets.
	#[must_use]
	pub fn total_sets(&self) -> u64 {
		self.total_sets
	}

	/// Returns the cache's total number of dels.
	#[must_use]
	pub fn total_dels(&self) -> u64 {
		self.total_dels
	}

	/// Returns the cache's current miss ratio.
	#[must_use]
	pub fn miss_ratio(&self) -> f64 {
		if self.total_gets == 0 {
			return 1.0;
		}

		1.0 - self.total_hits as f64 / self.total_gets as f64
	}

	/// Returns the cache's configured eviction policies.
	#[must_use]
	pub fn policies(&self) -> &[PaperPolicy] {
		&self.policies
	}

	/// Returns the cache's current eviction policy.
	#[must_use]
	pub fn policy(&self) -> PaperPolicy {
		self.policy
	}

	/// Returns `true` if the cache is configured to automatically
	/// switch eviction policies.
	#[must_use]
	pub fn is_auto_policy(&self) -> bool {
		self.is_auto_policy
	}

	/// Returns the cache's current uptime.
	#[must_use]
	pub fn uptime(&self) -> u64 {
		time::timestamp() - self.start_time
	}
}

/// This struct holds the basic statistical information about `PaperCache`
/// and allows for atomic updates of its fields.
impl AtomicStatus {
	pub fn new(
		max_size: CacheSize,
		policies: &[PaperPolicy],
		mut policy: PaperPolicy,
	) -> Result<Self, CacheError> {
		let policies: Arc<[PaperPolicy]> = policies.into();
		let is_auto_policy = policy.is_auto();

		if is_auto_policy {
			policy = PaperPolicy::Lfu;
		}

		let policy_index = get_policy_index(&policies, policy)?;

		let status = AtomicStatus {
			max_size: AtomicCacheSize::new(max_size),
			base_used_size: AtomicCacheSize::default(),
			num_objects: AtomicU64::default(),

			total_hits: AtomicU64::default(),
			total_gets: AtomicU64::default(),
			total_sets: AtomicU64::default(),
			total_dels: AtomicU64::default(),

			policies,
			policy_index: AtomicUsize::new(policy_index),
			is_auto_policy: AtomicBool::new(is_auto_policy),

			start_time: AtomicU64::new(time::timestamp()),
		};

		Ok(status)
	}

	#[must_use]
	pub fn max_size(&self) -> CacheSize {
		self.max_size.load(Ordering::Relaxed)
	}

	#[must_use]
	pub fn used_size(&self, policy: &PaperPolicy) -> CacheSize {
		let base_used_size = self.base_used_size.load(Ordering::Acquire);
		let num_objects = self.num_objects.load(Ordering::Acquire);
		let policy_overhead = get_policy_overhead(policy);

		base_used_size + num_objects * policy_overhead as CacheSize
	}

	#[must_use]
	pub fn policies(&self) -> &[PaperPolicy] {
		&self.policies
	}

	#[must_use]
	pub fn policy(&self) -> PaperPolicy {
		let policy_index = self.policy_index.load(Ordering::Relaxed);
		self.policies[policy_index]
	}

	#[must_use]
	pub fn is_auto_policy(&self) -> bool {
		self.is_auto_policy.load(Ordering::Relaxed)
	}

	pub fn incr_hits(&self) {
		self.total_gets.fetch_add(1, Ordering::Relaxed);
		self.total_hits.fetch_add(1, Ordering::Relaxed);
	}

	pub fn incr_misses(&self) {
		self.total_gets.fetch_add(1, Ordering::Relaxed);
	}

	pub fn incr_sets(&self) {
		self.total_sets.fetch_add(1, Ordering::Relaxed);
	}

	pub fn incr_dels(&self) {
		self.total_dels.fetch_add(1, Ordering::Relaxed);
	}

	pub fn set_max_size(&self, max_size: u64) {
		self.max_size.store(max_size, Ordering::Relaxed);
	}

	pub fn update_base_used_size(&self, delta: impl AsPrimitive<i64>) {
		let delta = delta.as_();

		if delta > 0 {
			self.base_used_size.fetch_add(delta.unsigned_abs(), Ordering::AcqRel);
		} else if delta < 0 {
			self.base_used_size.fetch_sub(delta.unsigned_abs(), Ordering::AcqRel);
		}
	}

	pub fn incr_num_objects(&self) {
		self.num_objects.fetch_add(1, Ordering::AcqRel);
	}

	pub fn decr_num_objects(&self) {
		self.num_objects.fetch_sub(1, Ordering::AcqRel);
	}

	pub fn set_policy(&self, policy: PaperPolicy) -> Result<(), CacheError> {
		if policy.is_auto() {
			self.is_auto_policy.store(true, Ordering::Relaxed);
			return Ok(());
		}

		let index = get_policy_index(&self.policies, policy)?;

		self.policy_index.store(index, Ordering::Relaxed);
		self.is_auto_policy.store(false, Ordering::Relaxed);

		Ok(())
	}

	pub fn set_auto_policy(&self, policy: PaperPolicy) -> Result<(), CacheError> {
		if policy.is_auto() {
			error!("Attempting to set recursive auto policy");
			return Err(CacheError::Internal);
		}

		let index = get_policy_index(&self.policies, policy)?;
		self.policy_index.store(index, Ordering::Relaxed);

		Ok(())
	}

	#[must_use]
	pub fn exceeds_max_size(&self, size: impl AsPrimitive<u64>) -> bool {
		size.as_() > self.max_size.load(Ordering::Relaxed)
	}

	pub fn clear(&self) {
		self.base_used_size.store(0, Ordering::Release);
		self.num_objects.store(0, Ordering::Release);

		self.total_hits.store(0, Ordering::Relaxed);
		self.total_gets.store(0, Ordering::Relaxed);
		self.total_sets.store(0, Ordering::Relaxed);
		self.total_dels.store(0, Ordering::Relaxed);
	}

	pub fn try_to_status(&self) -> Result<Status, CacheError> {
		let policy = self.policy();

		let Ok(rss) = mem::rss(None) else {
			error!("Could not get RSS");
			return Err(CacheError::Internal);
		};

		let Ok(hwm) = mem::hwm(None) else {
			error!("Could not get HWM");
			return Err(CacheError::Internal);
		};

		let status = Status {
			pid: process::id(),

			max_size: self.max_size(),
			used_size: self.used_size(&policy),
			num_objects: self.num_objects.load(Ordering::Acquire),

			rss,
			hwm,

			total_hits: self.total_hits.load(Ordering::Relaxed),
			total_gets: self.total_gets.load(Ordering::Relaxed),
			total_sets: self.total_sets.load(Ordering::Relaxed),
			total_dels: self.total_dels.load(Ordering::Relaxed),

			policies: self.policies.clone(),
			policy: self.policies[self.policy_index.load(Ordering::Relaxed)],
			is_auto_policy: self.is_auto_policy.load(Ordering::Relaxed),

			start_time: self.start_time.load(Ordering::Relaxed),
		};

		Ok(status)
	}
}

fn get_policy_index(
	policies: &[PaperPolicy],
	policy: PaperPolicy,
) -> Result<usize, CacheError> {
	let maybe_index = policies
		.iter()
		.position(|configured_policy| configured_policy.eq(&policy));

	match maybe_index {
		Some(index) => Ok(index),

		None => {
			error!("Could not find policy index");
			Err(CacheError::Internal)
		},
	}
}

#[cfg(test)]
mod tests {
	use std::sync::atomic::Ordering;

	use crate::{
		PaperPolicy,
		status::AtomicStatus,
	};

	#[test]
	fn it_clears_atomic_status() {
		let status = AtomicStatus::new(
			1000,
			&[PaperPolicy::Lfu],
			PaperPolicy::Lfu,
		).expect("Could not initialize atomic status");

		status.update_base_used_size(1);
		status.incr_num_objects();
		status.incr_hits();
		status.incr_sets();
		status.incr_dels();

		assert_eq!(status.base_used_size.load(Ordering::Acquire), 1);
		assert_eq!(status.num_objects.load(Ordering::Acquire), 1);
		assert_eq!(status.total_gets.load(Ordering::Relaxed), 1);
		assert_eq!(status.total_hits.load(Ordering::Relaxed), 1);
		assert_eq!(status.total_sets.load(Ordering::Relaxed), 1);
		assert_eq!(status.total_dels.load(Ordering::Relaxed), 1);

		status.clear();

		assert_eq!(status.base_used_size.load(Ordering::Acquire), 0);
		assert_eq!(status.num_objects.load(Ordering::Acquire), 0);
		assert_eq!(status.total_gets.load(Ordering::Relaxed), 0);
		assert_eq!(status.total_hits.load(Ordering::Relaxed), 0);
		assert_eq!(status.total_sets.load(Ordering::Relaxed), 0);
		assert_eq!(status.total_dels.load(Ordering::Relaxed), 0);
	}
}
