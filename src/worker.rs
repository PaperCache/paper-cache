mod ttl;

use std::sync::{Arc, Mutex};
use std::fmt::Display;
use std::hash::Hash;
use crate::object::MemSize;
use crate::cache::Cache;

pub const TIME_INCREMENT: u64 = 500;

pub trait Worker<K, V>
where
	Self: 'static + Send,
	K: 'static + Eq + Hash + Copy + Display + Sync,
	V: 'static + Clone + Sync + MemSize,
{
	fn new(_: Arc<Mutex<Cache<K, V>>>) -> Self where Self: Sized;
	fn start(&self);
}

pub use crate::worker::ttl::TtlWorker;
