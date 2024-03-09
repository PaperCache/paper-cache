use std::{
	hash::{Hash, BuildHasher},
	time::Duration,
	thread,
};

use crossbeam_channel::Receiver;

use crate::{
	paper_cache::{CacheSize, ObjectMapRef, StatsRef, erase},
	object::{MemSize, ObjectSize},
	worker::{Worker, WorkerEvent, WorkerReceiver},
	policy::{Policy, PolicyStack, PolicyStackType},
};

pub struct PolicyWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
	S: Default + Clone + BuildHasher,
{
	listener: Receiver<WorkerEvent<K>>,

	objects: ObjectMapRef<K, V, S>,
	stats: StatsRef,

	max_cache_size: CacheSize,

	policy_stacks: Vec<PolicyStackType<K>>,
	policy_index: usize,
}

impl<K, V, S> Worker<K, V, S> for PolicyWorker<K, V, S>
where
	Self: 'static + Send,
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
	S: Default + Clone + BuildHasher,
{
	fn run(&mut self) {
		loop {
			let events = self.listener
				.try_iter()
				.collect::<Vec<WorkerEvent<K>>>();

			for event in events.iter() {
				match *event {
					WorkerEvent::Get(key) => self.handle_get(key),
					WorkerEvent::Set(key, size, _) => self.handle_set(key, size),
					WorkerEvent::Del(key, _) => self.handle_del(key),
					WorkerEvent::Wipe => self.handle_wipe(),

					WorkerEvent::Resize(max_cache_size) => self.max_cache_size = max_cache_size,

					WorkerEvent::Policy(policy) => {
						self.policy_index = self.policy_stacks
							.iter()
							.position(|policy_stack| policy_stack.is_policy(policy))
							.unwrap_or(0);
					},
				}
			}

			let policy_stack = &mut self.policy_stacks[self.policy_index];

			while let Some(key) = policy_stack.eviction(self.max_cache_size) {
				erase(&self.objects, &self.stats, key).ok();
			}

			thread::sleep(Duration::from_millis(1));
		}
	}
}

impl<K, V, S> PolicyWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
	S: Default + Clone + BuildHasher,
{
	pub fn new(
		listener: WorkerReceiver<K>,
		objects: ObjectMapRef<K, V, S>,
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
			listener,

			objects,
			stats,

			max_cache_size,

			policy_stacks,
			policy_index,
		}
	}

	fn handle_get(&mut self, key: K) {
		for policy_stack in self.policy_stacks.iter_mut() {
			policy_stack.update(key);
		}
	}

	fn handle_set(&mut self, key: K, size: ObjectSize) {
		for policy_stack in self.policy_stacks.iter_mut() {
			policy_stack.insert(key, size);
		}
	}

	fn handle_del(&mut self, key: K) {
		for policy_stack in self.policy_stacks.iter_mut() {
			policy_stack.remove(key);
		}
	}

	fn handle_wipe(&mut self) {
		for policy_stack in self.policy_stacks.iter_mut() {
			policy_stack.clear();
		}
	}
}

unsafe impl<K, V, S> Send for PolicyWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
	S: Default + Clone + BuildHasher,
{}
