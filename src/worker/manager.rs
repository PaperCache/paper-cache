use std::{
	thread,
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
		TraceWorker,
	},
};

pub struct WorkerManager<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Sync + TypeSize + ReadChunk + WriteChunk,
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
	K: 'static + Copy + Eq + Hash + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{
	fn run(&mut self) -> Result<(), CacheError> {
		loop {
			let Ok(event) = self.listener.recv() else {
				return Ok(());
			};

			for worker in self.workers.iter() {
				worker.send(event.clone())
					.map_err(|_| CacheError::Internal)?;
			}
		}
	}
}

impl<K, V, S> WorkerManager<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: 'static + Default + Clone + BuildHasher,
{
	pub fn new(
		listener: WorkerReceiver<K>,
		objects: &ObjectMapRef<K, V, S>,
		stats: &StatsRef,
		overhead_manager: &OverheadManagerRef,
		policies: &[PaperPolicy],
	) -> Self {
		let (policy_worker, policy_listener) = unbounded();
		let (ttl_worker, ttl_listener) = unbounded();
		let (trace_worker, trace_listener) = unbounded();

		let traces = Arc::new(RwLock::new(VecDeque::new()));

		register_worker(PolicyWorker::<K, V, S>::new(
			policy_listener,
			objects.clone(),
			stats.clone(),
			overhead_manager.clone(),
			policies,
			traces.clone(),
		));

		register_worker(TtlWorker::<K, V, S>::new(
			ttl_listener,
			objects.clone(),
			stats.clone(),
			overhead_manager.clone(),
		));


		register_worker(TraceWorker::<K, V, S>::new(
			trace_listener,
			traces.clone(),
		));

		let workers: Arc<Box<[WorkerSender<K>]>> = Arc::new(Box::new([
			policy_worker,
			ttl_worker,
			trace_worker,
		]));

		WorkerManager {
			listener,
			workers,

			_v_marker: PhantomData,
			_s_marker: PhantomData,
		}
	}
}

fn register_worker<K, V, S>(mut worker: impl Worker<K, V, S>)
where
	K: 'static + Copy + Eq + Hash + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{
	thread::spawn(move || worker.run());
}

unsafe impl<K, V, S> Send for WorkerManager<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{}
