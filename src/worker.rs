mod manager;
mod policy;
mod ttl;

use std::hash::{Hash, BuildHasher};
use crossbeam_channel::{Sender, Receiver};

use crate::{
	paper_cache::CacheSize,
	error::CacheError,
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

	Ttl(K, ExpireTime, ExpireTime),

	Wipe,

	Resize(CacheSize),
	Policy(Policy),
}

pub trait Worker<K, V, S>
where
	Self: 'static + Send,
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
	S: Default + Clone + BuildHasher,
{
	fn run(&mut self) -> Result<(), CacheError>;
}

pub use crate::worker::{
	manager::WorkerManager,
	policy::PolicyWorker,
	ttl::TtlWorker,
};
