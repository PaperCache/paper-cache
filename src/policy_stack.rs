mod lfu_stack;
mod lru_stack;
mod mru_stack;
mod fifo_stack;

use std::{
	rc::Rc,
	hash::Hash,
};

pub trait PolicyStack<K>
where
	K: Eq + Hash,
{
	fn new() -> Self where Self: Sized;

	fn insert(&mut self, _: &Rc<K>);
	fn update(&mut self, _: &Rc<K>) {}
	fn remove(&mut self, _: &K);

	fn clear(&mut self);

	fn get_eviction(&mut self) -> Option<Rc<K>>;
}

pub use crate::policy_stack::{
	lfu_stack::*,
	lru_stack::*,
	mru_stack::*,
	fifo_stack::*,
};
