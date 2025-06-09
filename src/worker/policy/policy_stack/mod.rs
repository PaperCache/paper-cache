/*
 * Copyright (c) Kia Shakiba
 *
 * This source code is licensed under the GNU AGPLv3 license found in the
 * LICENSE file in the root directory of this source tree.
 */

mod lfu_stack;
mod fifo_stack;
mod clock_stack;
mod sieve_stack;
mod lru_stack;
mod mru_stack;
mod two_q_stack;
mod arc_stack;
mod s_three_fifo_stack;

use crate::{
	CacheSize,
	HashedKey,
	policy::PaperPolicy,
	object::ObjectSize,
	worker::policy::policy_stack::{
		lfu_stack::LfuStack,
		fifo_stack::FifoStack,
		clock_stack::ClockStack,
		sieve_stack::SieveStack,
		lru_stack::LruStack,
		mru_stack::MruStack,
		two_q_stack::TwoQStack,
		arc_stack::ArcStack,
		s_three_fifo_stack::SThreeFifoStack,
	},
};

pub trait PolicyStack
where
	Self: Send,
{
	fn is_policy(&self, policy: &PaperPolicy) -> bool;
	fn len(&self) -> usize;

	fn contains(&self, key: HashedKey) -> bool;
	fn insert(&mut self, key: HashedKey, size: ObjectSize);
	fn update(&mut self, _key: HashedKey) {}
	fn remove(&mut self, key: HashedKey);

	fn resize(&mut self, _size: CacheSize) {}
	fn clear(&mut self);

	fn evict_one(&mut self) -> Option<HashedKey>;
}

pub fn init_policy_stack(policy: PaperPolicy, max_size: CacheSize) -> Box<dyn PolicyStack> {
	match policy {
		PaperPolicy::Auto => Box::new(LfuStack::default()),
		PaperPolicy::Lfu => Box::new(LfuStack::default()),
		PaperPolicy::Fifo => Box::new(FifoStack::default()),
		PaperPolicy::Clock => Box::new(ClockStack::default()),
		PaperPolicy::Sieve => Box::new(SieveStack::default()),
		PaperPolicy::Lru => Box::new(LruStack::default()),
		PaperPolicy::Mru => Box::new(MruStack::default()),
		PaperPolicy::TwoQ(k_in, k_out) => Box::new(TwoQStack::new(k_in, k_out, max_size)),
		PaperPolicy::Arc => Box::new(ArcStack::new(max_size)),
		PaperPolicy::SThreeFifo(ratio) => Box::new(SThreeFifoStack::new(ratio, max_size)),
	}
}
