/*
 * Copyright (c) Kia Shakiba
 *
 * This source code is licensed under the GNU AGPLv3 license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::cmp::Ordering;
use rayon::prelude::*;

use crate::{
	CacheSize,
	HashedKey,
	ObjectSize,
	policy::PaperPolicy,
	object::overhead::get_policy_overhead,
	worker::policy::{
		policy_stack::PolicyStack,
		mini_stack::MiniStack,
	},
};

// the sampling modulus must be a power of 2
const MINI_SAMPLING_MODULUS: u64 = 16_777_216;
const MINI_SAMPLING_THRESHOLD: u64 = 16_777;

pub struct MiniStackManager {
	mini_stacks: Box<[MiniStack]>,
	total_gets: u64,
}

impl MiniStackManager {
	pub fn new(policies: &[PaperPolicy], cache_size: CacheSize) -> Self {
		let mini_size = get_mini_stack_size(cache_size);

		let mini_stacks = policies
			.iter()
			.map(|policy| MiniStack::new(*policy, mini_size))
			.collect::<Box<[_]>>();

		MiniStackManager {
			mini_stacks,
			total_gets: 0,
		}
	}

	pub fn get_index(&mut self, policy: &PaperPolicy) -> usize {
		self.mini_stacks
			.iter()
			.position(|mini_stack| mini_stack.is_policy(policy))
			.unwrap_or(0)
	}

	pub fn get_eviction(&mut self, index: usize) -> Option<HashedKey> {
		self.mini_stacks[index].evict_one()
	}

	pub fn handle_get(&mut self, key: HashedKey) {
		self.total_gets += 1;

		if !should_sample(key) {
			return;
		}

		self.mini_stacks
			.par_iter_mut()
			.for_each(|mini_stack| mini_stack.update_with_count(key));
	}

	pub fn handle_set(&mut self, key: HashedKey, size: ObjectSize) {
		if !should_sample(key) {
			return;
		}

		self.mini_stacks
			.par_iter_mut()
			.for_each(|mini_stack| mini_stack.insert(key, size));
	}

	pub fn handle_del(&mut self, key: HashedKey) {
		if !should_sample(key) {
			return;
		}

		self.mini_stacks
			.par_iter_mut()
			.for_each(|mini_stack| mini_stack.remove(key));
	}

	pub fn handle_resize(&mut self, size: CacheSize) {
		let mini_size = get_mini_stack_size(size);

		self.mini_stacks
			.par_iter_mut()
			.for_each(|mini_stack| mini_stack.resize(mini_size));
	}

	pub fn handle_wipe(&mut self) {
		self.mini_stacks
			.par_iter_mut()
			.for_each(|mini_stack| mini_stack.clear());

		self.total_gets = 0;
	}

	pub fn apply_evictions(&mut self, exclude_index: usize, evictions: Vec<HashedKey>) {
		self.mini_stacks
			.par_iter_mut()
			.enumerate()
			.filter(|(index, _)| *index != exclude_index)
			.for_each(|(_, mini_stack)| {
				for key in &evictions {
					mini_stack.remove(*key);
				}
			});
	}

	pub fn get_optimal_policy(&self, current_policy: &PaperPolicy) -> Option<PaperPolicy> {
		let sampling_ratio = MINI_SAMPLING_THRESHOLD as f64 / MINI_SAMPLING_MODULUS as f64;
		let expected_count = self.total_gets as f64 * sampling_ratio;

		let current_miss_ratio = self.mini_stacks
			.iter()
			.find_map(|mini_stack| {
				if !mini_stack.is_policy(current_policy) {
					return None;
				}

				Some(mini_stack.miss_ratio(expected_count))
			})?;

		let optimal_mini_stack = self.mini_stacks
			.iter()
			.min_by(|a, b| {
				match a.miss_ratio(expected_count).total_cmp(&b.miss_ratio(expected_count)) {
					Ordering::Equal => {
						// the two mini stacks have the same miss ratios, so
						// select the one with the lower memory overhead
						let a_overhead = get_policy_overhead(&a.policy());
						let b_overhead = get_policy_overhead(&b.policy());

						a_overhead.cmp(&b_overhead)
					},

					cmp => cmp,
				}
			})?;

		if optimal_mini_stack.miss_ratio(expected_count) < current_miss_ratio {
			// make sure we only switch to a different policy that performs better
			// than the current policy
			Some(optimal_mini_stack.policy())
		} else {
			None
		}
	}
}

fn should_sample(key: HashedKey) -> bool {
	// this optimization only works if the sampling modulus is a power of 2
	key & (MINI_SAMPLING_MODULUS - 1) < MINI_SAMPLING_THRESHOLD
}

fn get_mini_stack_size(size: CacheSize) -> CacheSize {
	let ratio = MINI_SAMPLING_THRESHOLD as f64 / MINI_SAMPLING_MODULUS as f64;
	(size as f64 * ratio) as u64
}
