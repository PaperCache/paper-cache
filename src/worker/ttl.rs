use std::{
	hash::Hash,
	thread,
	time::Duration,
};

use crossbeam_channel::Receiver;

use crate::{
	paper_cache::{ObjectMapRef, StatsRef},
	object::MemSize,
	worker::{Worker, WorkerEvent},
};

pub struct TtlWorker<K, V>
where
	K: 'static + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{
	events: Receiver<WorkerEvent<K>>,
	objects: ObjectMapRef<K, V>,
}

impl<K, V> Worker<K, V> for TtlWorker<K, V>
where
	Self: 'static + Send,
	K: 'static + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{
	fn new(
		events: Receiver<WorkerEvent<K>>,
		objects: ObjectMapRef<K, V>,
		_: StatsRef,
	) -> Self {
		TtlWorker {
			objects,
			events,
		}
	}

	fn start(&self) {
		loop {
			if let Ok(event) = self.events.try_recv() {
				// handle event here
			}

			thread::sleep(Duration::from_millis(1));
		}
	}
}

unsafe impl<K, V> Send for TtlWorker<K, V>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{}
