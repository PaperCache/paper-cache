use std::{
	sync::{Arc, RwLock},
	hash::{Hash, BuildHasher},
	collections::VecDeque,
	marker::PhantomData,
};

use typesize::TypeSize;
use crossbeam_channel::unbounded;
use kwik::file::binary::{ReadChunk, WriteChunk};

use crate::{
	cache::{ObjectMapRef, StatsRef, OverheadManagerRef},
	error::CacheError,
	policy::PaperPolicy,
	worker::{
		Worker,
		WorkerSender,
		WorkerReceiver,
		PolicyWorker,
		TtlWorker,
		register_worker,
	},
};

pub struct WorkerManager<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{
	listener: WorkerReceiver<K>,
	workers: Arc<Box<[WorkerSender<K>]>>,

	_v_marker: PhantomData<V>,
	_s_marker: PhantomData<S>,
}

impl<K, V, S> Worker<K, V, S> for WorkerManager<K, V, S>
where
	Self: 'static + Send,
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{
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

impl<K, V, S> WorkerManager<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: 'static + Default + Clone + BuildHasher,
{
	pub fn new(
		listener: WorkerReceiver<K>,
		objects: &ObjectMapRef<K, V, S>,
		stats: &StatsRef,
		overhead_manager: &OverheadManagerRef,
		policy: PaperPolicy,
	) -> Self {
		let (policy_worker, policy_listener) = unbounded();
		let (ttl_worker, ttl_listener) = unbounded();

		let traces = Arc::new(RwLock::new(VecDeque::new()));

		register_worker(PolicyWorker::<K, V, S>::new(
			policy_listener,
			objects.clone(),
			stats.clone(),
			overhead_manager.clone(),
			policy,
			traces.clone(),
		));

		register_worker(TtlWorker::<K, V, S>::new(
			ttl_listener,
			objects.clone(),
			stats.clone(),
			overhead_manager.clone(),
		));

		let workers: Arc<Box<[WorkerSender<K>]>> = Arc::new(Box::new([
			policy_worker,
			ttl_worker,
		]));

		WorkerManager {
			listener,
			workers,

			_v_marker: PhantomData,
			_s_marker: PhantomData,
		}
	}
}

unsafe impl<K, V, S> Send for WorkerManager<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{}
