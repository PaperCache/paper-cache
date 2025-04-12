mod expiries;

use std::{
	thread,
	time::{Instant, Duration},
};

use typesize::TypeSize;

use crate::{
	ObjectMapRef,
	StatsRef,
	OverheadManagerRef,
	EraseKey,
	erase,
	error::CacheError,
	worker::{
		Worker,
		WorkerEvent,
		WorkerReceiver,
		ttl::expiries::Expiries,
	},
};

pub struct TtlWorker<K, V> {
	listener: WorkerReceiver,

	objects: ObjectMapRef<K, V>,
	stats: StatsRef,
	overhead_manager: OverheadManagerRef,

	expiries: Expiries,
}

impl<K, V> Worker for TtlWorker<K, V>
where
	Self: 'static + Send,
	K: Eq + TypeSize,
	V: TypeSize,
{
	fn run(&mut self) -> Result<(), CacheError> {
		loop {
			let now = Instant::now();

			for event in self.listener.try_iter() {
				match event {
					WorkerEvent::Set(key, _, expiry, old_info) => {
						if let Some((_, old_expiry)) = old_info {
							self.expiries.remove(key, old_expiry);
						}

						self.expiries.insert(key, expiry);
					},

					WorkerEvent::Del(key, expiry) => self.expiries.remove(key, expiry),

					WorkerEvent::Ttl(key, old_expiry, new_expiry) => {
						self.expiries.remove(key, old_expiry);
						self.expiries.insert(key, new_expiry);
					},

					WorkerEvent::Wipe => self.expiries.clear(),

					_ => {},
				}
			}

			while let Some(key) = self.expiries.pop_expired(now) {
				erase(
					&self.objects,
					&self.stats,
					&self.overhead_manager,
					Some(EraseKey::Hashed(key)),
				).ok();
			}

			let delay_ms = match self.expiries.has_within(2) {
				true => 1,
				false => 1000,
			};

			thread::sleep(Duration::from_millis(delay_ms));
		}
	}
}

impl<K, V> TtlWorker<K, V> {
	pub fn new(
		listener: WorkerReceiver,
		objects: ObjectMapRef<K, V>,
		stats: StatsRef,
		overhead_manager: OverheadManagerRef,
	) -> Self {
		TtlWorker {
			listener,

			objects,
			stats,
			overhead_manager,

			expiries: Expiries::default(),
		}
	}
}

unsafe impl<K, V> Send for TtlWorker<K, V> {}
