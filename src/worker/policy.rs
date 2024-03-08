use std::{
	hash::Hash,
	thread,
	time::Duration,
};

use crossbeam_channel::Receiver;

use crate::{
	paper_cache::{CacheSize, ObjectMapRef, StatsRef, erase},
	object::MemSize,
	worker::{Worker, WorkerEvent, WorkerReceiver},
	policy::{Policy, PolicyStack, PolicyStackType},
};

pub struct PolicyWorker<K, V>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{
	listener: Option<Receiver<WorkerEvent<K>>>,

	objects: ObjectMapRef<K, V>,
	stats: StatsRef,

	max_cache_size: CacheSize,

	policy_stacks: Vec<PolicyStackType<K>>,
	policy_index: usize,
}

impl<K, V> Worker<K, V> for PolicyWorker<K, V>
where
	Self: 'static + Send,
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{
	fn run(&mut self) {
		loop {
			if let Some(listener) = &self.listener {
				for event in listener.try_iter() {
					match event {
						WorkerEvent::Get(key) => {
							for policy_stack in self.policy_stacks.iter_mut() {
								policy_stack.update(key);
							}
						},

						WorkerEvent::Set(key, size, _) => {
							for policy_stack in self.policy_stacks.iter_mut() {
								policy_stack.insert(key, size);
							}
						},

						WorkerEvent::Del(key, _) => {
							for policy_stack in self.policy_stacks.iter_mut() {
								policy_stack.remove(key);
							}
						},

						WorkerEvent::Wipe => {
							for policy_stack in self.policy_stacks.iter_mut() {
								policy_stack.clear();
							}
						},

						WorkerEvent::Resize(max_cache_size) => {
							self.max_cache_size = max_cache_size;
						},

						WorkerEvent::Policy(policy) => {
							self.policy_index = self.policy_stacks
								.iter()
								.position(|policy_stack| policy_stack.is_policy(policy))
								.unwrap_or(0);
						},
					}
				}
			}

			let policy_stack = &mut self.policy_stacks[self.policy_index];

			while let Some(key) = policy_stack.eviction(self.max_cache_size) {
				erase(&self.objects, &self.stats, key).ok();
			}

			thread::sleep(Duration::from_millis(1));
		}
	}

	fn listen(&mut self, listener: WorkerReceiver<K>) {
		self.listener = Some(listener);
	}
}

impl<K, V> PolicyWorker<K, V>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{
	pub fn new(
		objects: ObjectMapRef<K, V>,
		stats: StatsRef,
		policies: Vec<Policy>,
	) -> Self {
		let max_cache_size = stats
			.read().unwrap()
			.get_max_size();

		let policy_stacks = policies
			.iter()
			.map(|policy| policy.as_policy_stack_type())
			.collect();

		let policy_index = 0;

		PolicyWorker {
			listener: None,

			objects,
			stats,

			max_cache_size,

			policy_stacks,
			policy_index,
		}
	}
}

unsafe impl<K, V> Send for PolicyWorker<K, V>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
{}
