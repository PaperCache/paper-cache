use std::sync::{Arc, Mutex};
use std::fmt::Display;
use std::hash::Hash;
use std::thread;
use std::time::Duration;
use crate::object::MemSize;
use crate::cache::Cache;
use crate::worker::{Worker, TIME_INCREMENT};

pub struct TtlWorker<K, V>
where
	K: 'static + Eq + Hash + Copy + Display + Sync,
	V: 'static + Clone + Sync + MemSize,
{
	cache: Arc<Mutex<Cache<K, V>>>,
}

impl<K, V> Worker<K, V> for TtlWorker<K, V>
where
	K: 'static + Eq + Hash + Copy + Display + Sync,
	V: 'static + Clone + Sync + MemSize,
{
	fn new(cache: Arc<Mutex<Cache<K, V>>>) -> Self {
		TtlWorker {
			cache,
		}
	}

	fn start(&self) {
		loop {
			thread::sleep(Duration::from_millis(TIME_INCREMENT));

			let mut cache = self.cache.lock().unwrap();
			cache.prune_expired();
		}
	}
}
