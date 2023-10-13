mod ttl;

use std::{
	sync::{Arc, Mutex},
	fmt::Display,
	hash::Hash,
};

use crate::{
	object::MemSize,
	cache::Cache,
};

pub trait Worker<K, V>
where
	Self: 'static + Send,
	K: 'static + Eq + Hash + Clone + Display + Sync,
	V: 'static + Clone + Sync + MemSize,
{
	fn new(_: Arc<Mutex<Cache<K, V>>>) -> Self where Self: Sized;
	fn start(&self);
}

pub use crate::worker::ttl::TtlWorker;
