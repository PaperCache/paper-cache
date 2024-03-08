mod ttl;

use std::hash::Hash;
use crossbeam_channel::Receiver;

use crate::{
	paper_cache::{ObjectMapRef, StatsRef},
	object::MemSize,
};

pub enum WorkerEvent<K> {
	Get(K),
	Set(K, u64, Option<u32>),
	Del(K),
	Wipe,
}

pub trait Worker<K, V>
where
	Self: 'static + Send,
	K: 'static + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{
	fn new(
		events: Receiver<WorkerEvent<K>>,
		objects: ObjectMapRef<K, V>,
		stats: StatsRef,
	) -> Self where Self: Sized;

	fn start(&self);
}

pub use crate::worker::ttl::TtlWorker;
