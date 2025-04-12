mod manager;
mod policy;
mod ttl;

use std::thread;
use crossbeam_channel::{Sender, Receiver};

use crate::{
	CacheSize,
	HashedKey,
	error::CacheError,
	object::{ObjectSize, ExpireTime},
	policy::PaperPolicy,
};

pub type WorkerSender = Sender<WorkerEvent>;
pub type WorkerReceiver = Receiver<WorkerEvent>;

#[derive(Clone)]
pub enum WorkerEvent {
	Get(HashedKey, bool),
	Set(HashedKey, ObjectSize, ExpireTime, Option<(ObjectSize, ExpireTime)>),
	Del(HashedKey, ExpireTime),

	Ttl(HashedKey, ExpireTime, ExpireTime),

	Wipe,

	Resize(CacheSize),
	Policy(PaperPolicy),
}

pub trait Worker
where
	Self: 'static + Send,
{
	fn run(&mut self) -> Result<(), CacheError>;
}

pub fn register_worker(mut worker: impl Worker) {
	thread::spawn(move || worker.run());
}

pub use crate::worker::{
	manager::WorkerManager,
	policy::PolicyWorker,
	ttl::TtlWorker,
};
