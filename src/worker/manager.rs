use std::sync::Arc;
use typesize::TypeSize;
use crossbeam_channel::unbounded;

use crate::{
	ObjectMapRef,
	StatsRef,
	OverheadManagerRef,
	error::CacheError,
	worker::{
		Worker,
		WorkerSender,
		WorkerReceiver,
		PolicyWorker,
		TtlWorker,
		register_worker,
	},
};

pub struct WorkerManager {
	listener: WorkerReceiver,
	workers: Arc<Box<[WorkerSender]>>,
}

impl Worker for WorkerManager {
	fn run(&mut self) -> Result<(), CacheError> {
		loop {
			let Ok(event) = self.listener.recv() else {
				return Ok(());
			};

			for worker in self.workers.iter() {
				worker.try_send(event.clone())
					.map_err(|_| CacheError::Internal)?;
			}
		}
	}
}

impl WorkerManager {
	pub fn new<K, V>(
		listener: WorkerReceiver,
		objects: &ObjectMapRef<K, V>,
		stats: &StatsRef,
		overhead_manager: &OverheadManagerRef,
	) -> Result<Self, CacheError>
	where
		K: 'static + Eq + TypeSize,
		V: 'static + TypeSize,
	{
		let (policy_worker, policy_listener) = unbounded();
		let (ttl_worker, ttl_listener) = unbounded();

		register_worker(PolicyWorker::<K, V>::new(
			policy_listener,
			objects.clone(),
			stats.clone(),
			overhead_manager.clone(),
		)?);

		register_worker(TtlWorker::<K, V>::new(
			ttl_listener,
			objects.clone(),
			stats.clone(),
			overhead_manager.clone(),
		));

		let workers: Arc<Box<[WorkerSender]>> = Arc::new(Box::new([
			policy_worker,
			ttl_worker,
		]));

		let manager = WorkerManager {
			listener,
			workers,
		};

		Ok(manager)
	}
}

unsafe impl Send for WorkerManager {}
