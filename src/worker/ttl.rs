use std::{
	sync::{Arc, Mutex},
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
	K: 'static + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{
	cache: Arc<Mutex<Cache<K, V>>>,
}

impl<K, V> Worker<K, V> for TtlWorker<K, V>
where
	K: 'static + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
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
