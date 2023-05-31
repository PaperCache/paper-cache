mod lru_stack;
mod mru_stack;

use std::hash::Hash;

pub trait PolicyStack<K>
where
	K: Eq + Hash + Copy,
{
	fn new() -> Self where Self: Sized;

	fn insert(&mut self, _: &K);
	fn update(&mut self, _: &K);
	fn remove(&mut self, _: &K);

	fn clear(&mut self);

	fn get_eviction(&mut self) -> Option<K>;
}

pub use crate::policy_stack::lru_stack::*;
pub use crate::policy_stack::mru_stack::*;
