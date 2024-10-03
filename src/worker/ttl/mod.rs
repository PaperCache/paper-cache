mod expiries;

use std::{
	thread,
	hash::{Hash, BuildHasher},
	time::Duration,
};

use typesize::TypeSize;

use kwik::{
	time,
	file::binary::{ReadChunk, WriteChunk},
};

use crate::{
	cache::{ObjectMapRef, StatsRef, OverheadManagerRef, erase},
	error::CacheError,
	worker::{
		Worker,
		WorkerEvent,
		WorkerReceiver,
		ttl::expiries::Expiries,
	},
};

pub struct TtlWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Clone + BuildHasher,
{
	listener: WorkerReceiver<K>,

	objects: ObjectMapRef<K, V, S>,
	stats: StatsRef,
	overhead_manager: OverheadManagerRef,

	expiries: Expiries<K, S>,
}

impl<K, V, S> Worker<K, V, S> for TtlWorker<K, V, S>
where
	Self: 'static + Send,
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Clone + BuildHasher,
{
	fn run(&mut self) -> Result<(), CacheError> {
		loop {
			let now = time::timestamp();

			for event in self.listener.try_iter() {
				match event {
					WorkerEvent::Set(key, _, expiry, _) => self.expiries.insert(key, expiry),
					WorkerEvent::Del(key, _, expiry) => self.expiries.remove(key, expiry),

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
					erase(
						&self.objects,
						&self.stats,
						&self.overhead_manager,
						Some(key),
					).ok();
				}
			}

			let delay = match self.expiries.has_within(2) {
				true => 1,
				false => 1000,
			};

			thread::sleep(Duration::from_millis(delay));
		}
	}
}

impl<K, V, S> TtlWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Clone + BuildHasher,
{
	pub fn with_hasher(
		listener: WorkerReceiver<K>,
		objects: ObjectMapRef<K, V, S>,
		stats: StatsRef,
		overhead_manager: OverheadManagerRef,
		hasher: S,
	) -> Self {
		TtlWorker {
			listener,

			objects,
			stats,
			overhead_manager,

			expiries: Expiries::with_hasher(hasher),
		}
	}
}

unsafe impl<K, V, S> Send for TtlWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{}
