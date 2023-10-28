mod ttl;

use std::{
	sync::{Arc, Mutex},
	hash::Hash,
};

use crate::{
	object::MemSize,
	cache::Cache,
};

pub trait Worker<K, V>
where
	Self: 'static + Send,
	K: 'static + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{
	fn new(_: Arc<Mutex<Cache<K, V>>>) -> Self where Self: Sized;
	fn start(&self);
}

pub use crate::worker::ttl::TtlWorker;
