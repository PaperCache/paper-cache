/*
 * Copyright (c) Kia Shakiba
 *
 * This source code is licensed under the GNU AGPLv3 license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::sync::Arc;
use typesize::TypeSize;
use crossbeam_channel::unbounded;
use log::error;

use crate::{
	ObjectMapRef,
	StatusRef,
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
				if let Err(err) = worker.try_send(event.clone()) {
					error!("Could not send event to worker: {err:?}");
					return Err(CacheError::Internal);
				}
			}
		}
	}
}

impl WorkerManager {
	pub fn new<K, V>(
		listener: WorkerReceiver,
		objects: &ObjectMapRef<K, V>,
		status: &StatusRef,
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
			status.clone(),
			overhead_manager.clone(),
		)?);

		register_worker(TtlWorker::<K, V>::new(
			ttl_listener,
			objects.clone(),
			status.clone(),
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
