use std::{
	hash::Hash,
	thread,
	time::Duration,
};

use kwik::utils;

use crate::{
	paper_cache::{ObjectMapRef, StatsRef, erase},
	object::MemSize,
	worker::{Worker, WorkerEvent, WorkerReceiver},
	expiries::Expiries,
};

pub struct TtlWorker<K, V>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{
	listener: Option<WorkerReceiver<K>>,

	objects: ObjectMapRef<K, V>,
	stats: StatsRef,

	expiries: Expiries<K>,
}

impl<K, V> Worker<K, V> for TtlWorker<K, V>
where
	Self: 'static + Send,
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{
	fn run(&mut self) {
		loop {
			let now = utils::timestamp();

			if let Some(listener) = &self.listener {
				for event in listener.try_iter() {
					match event {
						WorkerEvent::Set(key, _, expiry) => self.expiries.insert(key, expiry),
						WorkerEvent::Del(key, expiry) => self.expiries.remove(key, expiry),
						WorkerEvent::Wipe => self.expiries.clear(),

						_ => {},
					}
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

	fn listen(&mut self, listener: WorkerReceiver<K>) {
		self.listener = Some(listener);
	}
}

impl<K, V> TtlWorker<K, V>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{
	pub fn new(
		objects: ObjectMapRef<K, V>,
		stats: StatsRef,
	) -> Self {
		TtlWorker {
			listener: None,

			objects,
			stats,

			expiries: Expiries::default(),
		}
	}
}

unsafe impl<K, V> Send for TtlWorker<K, V>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{}
