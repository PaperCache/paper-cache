use std::{
	hash::{Hash, BuildHasher},
	time::Duration,
	thread,
};

use kwik::utils;

use crate::{
	paper_cache::{ObjectMapRef, StatsRef, erase},
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
	fn run(&mut self) {
		loop {
			let now = utils::timestamp();

			for event in self.listener.try_iter() {
				match event {
					WorkerEvent::Set(key, _, expiry) => self.expiries.insert(key, expiry),
					WorkerEvent::Del(key, expiry) => self.expiries.remove(key, expiry),
					WorkerEvent::Wipe => self.expiries.clear(),

					_ => {},
				}
			}

			if let Some(expired) = self.expiries.expired(now) {
				for key in expired {
					erase(&self.objects, &self.stats, key).ok();
				}
			}

			thread::sleep(Duration::from_millis(1));
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
