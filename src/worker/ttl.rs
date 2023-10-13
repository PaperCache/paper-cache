use std::{
	sync::{Arc, Mutex},
	fmt::Display,
	hash::Hash,
	thread,
	time::Duration,
};

use crate::{
	object::MemSize,
	cache::Cache,
	worker::Worker,
};

pub struct TtlWorker<K, V>
where
	K: 'static + Eq + Hash + Clone + Display + Sync,
	V: 'static + Clone + Sync + MemSize,
{
	cache: Arc<Mutex<Cache<K, V>>>,
}

impl<K, V> Worker<K, V> for TtlWorker<K, V>
where
	K: 'static + Eq + Hash + Clone + Display + Sync,
	V: 'static + Clone + Sync + MemSize,
{
	fn new(cache: Arc<Mutex<Cache<K, V>>>) -> Self {
		TtlWorker {
			cache,
		}
	}

	fn start(&self) {
		loop {
			thread::sleep(Duration::from_millis(500));

			let mut cache = self.cache.lock().unwrap();
			cache.prune_expired();
		}
	}
}
