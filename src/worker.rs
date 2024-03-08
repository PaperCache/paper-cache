mod policy;
mod ttl;

use std::hash::Hash;
use crossbeam_channel::{Sender, Receiver};

use crate::{
	paper_cache::CacheSize,
	object::{MemSize, ObjectSize, ExpireTime},
	policy::Policy,
};

pub type WorkerSender<K> = Sender<WorkerEvent<K>>;
pub type WorkerReceiver<K> = Receiver<WorkerEvent<K>>;

#[derive(Clone)]
pub enum WorkerEvent<K> {
	Get(K),
	Set(K, ObjectSize, ExpireTime),
	Del(K, ExpireTime),

	Wipe,

	Resize(CacheSize),
	Policy(Policy),
}

pub trait Worker<K, V>
where
	Self: 'static + Send,
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{
	fn run(&mut self);
	fn listen(&mut self, events: WorkerReceiver<K>);
}

pub use crate::worker::{
	policy::PolicyWorker,
	ttl::TtlWorker,
};
