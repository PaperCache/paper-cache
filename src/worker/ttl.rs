use std::sync::{Arc, Mutex};
use std::fmt::Display;
use std::hash::Hash;
use std::thread;
use std::time::Duration;
use crate::object::MemSize;
use crate::cache::Cache;
use crate::worker::TIME_INCREMENT;

pub fn worker<K, V>(cache: Arc<Mutex<Cache<K, V>>>)
where
	K: Eq + Hash + Copy + 'static + Display + Sync,
	V: 'static + Clone + Sync + MemSize,
{
	loop {
		thread::sleep(Duration::from_millis(TIME_INCREMENT));

		let mut cache = cache.lock().unwrap();
		cache.prune_expired();
	}
}
