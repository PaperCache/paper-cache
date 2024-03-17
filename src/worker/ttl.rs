use std::{
	hash::{Hash, BuildHasher},
	time::Duration,
	thread,
};

use kwik::utils;

use crate::{
	paper_cache::{ObjectMapRef, StatsRef, erase},
	error::CacheError,
	object::MemSize,
	worker::{Worker, WorkerEvent, WorkerReceiver},
	expiries::Expiries,
};

pub struct TtlWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
	S: Default + Clone + BuildHasher,
{
	listener: WorkerReceiver<K>,

	objects: ObjectMapRef<K, V, S>,
	stats: StatsRef,

	expiries: Expiries<K>,
}

impl<K, V, S> Worker<K, V, S> for TtlWorker<K, V, S>
where
	Self: 'static + Send,
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
	S: Default + Clone + BuildHasher,
{
	fn run(&mut self) -> Result<(), CacheError> {
		loop {
			let now = utils::timestamp();

			for event in self.listener.try_iter() {
				match event {
					WorkerEvent::Set(key, _, expiry) => self.expiries.insert(key, expiry),
					WorkerEvent::Del(key, expiry) => self.expiries.remove(key, expiry),

					WorkerEvent::Ttl(key, old_expiry, new_expiry) => {
						self.expiries.remove(key, old_expiry);
						self.expiries.insert(key, new_expiry);
					},

					WorkerEvent::Wipe => self.expiries.clear(),

					_ => {},
				}
			}

			if let Some(expired) = self.expiries.expired(now) {
				for key in expired {
					erase(&self.objects, &self.stats, key).ok();
				}
			}

			let delay = match self.expiries.is_empty() {
				true => 1000,
				false => 1,
			};

			thread::sleep(Duration::from_millis(delay));
		}
	}
}

impl<K, V, S> TtlWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
	S: Default + Clone + BuildHasher,
{
	pub fn new(
		listener: WorkerReceiver<K>,
		objects: ObjectMapRef<K, V, S>,
		stats: StatsRef,
	) -> Self {
		TtlWorker {
			listener,

			objects,
			stats,

			expiries: Expiries::default(),
		}
	}
}

unsafe impl<K, V, S> Send for TtlWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
	S: Default + Clone + BuildHasher,
{}
