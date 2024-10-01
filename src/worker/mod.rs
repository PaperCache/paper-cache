mod manager;
mod policy;
mod ttl;

use std::{
	thread,
	hash::{Hash, BuildHasher},
};

use typesize::TypeSize;
use crossbeam_channel::{Sender, Receiver};
use kwik::file::binary::{ReadChunk, WriteChunk};

use crate::{
	cache::CacheSize,
	error::CacheError,
	object::{ObjectSize, ExpireTime},
	policy::PaperPolicy,
};

pub type WorkerSender<K> = Sender<WorkerEvent<K>>;
pub type WorkerReceiver<K> = Receiver<WorkerEvent<K>>;

#[derive(Clone)]
pub enum WorkerEvent<K> {
	Get(K, bool),
	Set(K, ObjectSize, ExpireTime, Option<ObjectSize>),
	Del(K, ObjectSize, ExpireTime),

	Ttl(K, ExpireTime, ExpireTime),

	Wipe,

	Resize(CacheSize),
	Policy(PaperPolicy),
}

pub trait Worker<K, V, S>
where
	Self: 'static + Send,
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{
	fn run(&mut self) -> Result<(), CacheError>;
}

pub fn register_worker<K, V, S>(mut worker: impl Worker<K, V, S>)
where
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{
	thread::spawn(move || worker.run());
}

pub use crate::worker::{
	manager::WorkerManager,
	policy::PolicyWorker,
	ttl::TtlWorker,
};
