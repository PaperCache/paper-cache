use std::{
	sync::Arc,
	hash::{Hash, BuildHasher},
	marker::PhantomData,
	thread,
};

use crossbeam_channel::unbounded;

use crate::{
	paper_cache::{ObjectMapRef, StatsRef},
	error::CacheError,
	object::MemSize,
	policy::Policy,
	worker::{
		Worker,
		WorkerSender,
		WorkerReceiver,
		PolicyWorker,
		TtlWorker,
	},
};

pub struct WorkerManager<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
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
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
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
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
	S: 'static + Default + Clone + BuildHasher,
{
	pub fn new(
		listener: WorkerReceiver<K>,
		objects: ObjectMapRef<K, V, S>,
		stats: StatsRef,
		policies: &[Policy],
	) -> Self {
		let (policy_worker, policy_listener) = unbounded();
		let (ttl_worker, ttl_listener) = unbounded();

		register_worker(PolicyWorker::<K, V, S>::new(
			policy_listener,
			objects.clone(),
			stats.clone(),
			policies,
		));

		register_worker(TtlWorker::<K, V, S>::new(
			ttl_listener,
			objects.clone(),
			stats.clone(),
		));

		WorkerManager {
			listener,
			workers: Arc::new(Box::new([policy_worker, ttl_worker])),

			_v_marker: PhantomData,
			_s_marker: PhantomData,
		}
	}
}

fn register_worker<K, V, S>(mut worker: impl Worker<K, V, S>)
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
	S: Default + Clone + BuildHasher,
{
	thread::spawn(move || worker.run());
}

unsafe impl<K, V, S> Send for WorkerManager<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
	S: Default + Clone + BuildHasher,
{}
