use std::{
	hash::{Hash, BuildHasher},
	time::Duration,
	thread,
};

use crossbeam_channel::Receiver;
use kwik::utils;

use crate::{
	paper_cache::{CacheSize, ObjectMapRef, StatsRef, erase},
	error::CacheError,
	object::MemSize,
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
	used_cache_size: CacheSize,
	policy_stacks: Vec<PolicyStackType<K>>,
	policy_index: usize,

	last_set_time: Option<u64>,
}

impl<K, V, S> Worker<K, V, S> for PolicyWorker<K, V, S>
where
	Self: 'static + Send,
	K: 'static + Copy + Eq + Hash + Sync,
	V: 'static + Sync + MemSize,
	S: Default + Clone + BuildHasher,
{
	fn run(&mut self) -> Result<(), CacheError> {
		loop {
			let events = self.listener
				.try_iter()
				.collect::<Vec<WorkerEvent<K>>>();

			let mut has_current_set = false;

			for event in events {
				match event {
					WorkerEvent::Get(key) => self.handle_get(key),

					WorkerEvent::Set(key, size, _, old_size) => {
						self.handle_set(key);
						has_current_set = true;

						self.used_cache_size += size;

						if let Some(old_size) = old_size {
							self.used_cache_size -= old_size;
						}
					},

					WorkerEvent::Del(key, size, _) => {
						self.handle_del(key);
						self.used_cache_size -= size;
					},

					WorkerEvent::Wipe => {
						self.handle_wipe();
						self.used_cache_size = 0;
					},

					WorkerEvent::Resize(max_cache_size) => self.max_cache_size = max_cache_size,

					WorkerEvent::Policy(policy) => {
						self.policy_index = self.policy_stacks
							.iter()
							.position(|policy_stack| policy_stack.is_policy(policy))
							.unwrap_or(0);
					},

					_ => {},
				}
			}

			let policy_stack = &mut self.policy_stacks[self.policy_index];
			let mut evicted_keys = Vec::<K>::new();

			while self.used_cache_size > self.max_cache_size {
				if let Some(key) = policy_stack.eviction() {
					if let Ok(object) = erase(&self.objects, &self.stats, key) {
						self.used_cache_size -= object.size();
						evicted_keys.push(key);
					}
				}
			}

			for key in evicted_keys {
				for (index, policy_stack) in self.policy_stacks.iter_mut().enumerate() {
					if index == self.policy_index {
						continue;
					}

					policy_stack.remove(key);
				}
			}

			let now = utils::timestamp();

			let has_recent_set = self.last_set_time
				.is_some_and(|last_set_time| now - last_set_time <= 5000);

			if has_current_set {
				self.last_set_time = Some(now);
			}

			let delay = match has_recent_set {
				true => 1,
				false => 1000,
			};

			thread::sleep(Duration::from_millis(delay));
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
		policies: &[Policy],
	) -> Self {
		let max_cache_size = stats.get_max_size();

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
			used_cache_size: 0,

			policy_stacks,
			policy_index,

			last_set_time: None,
		}
	}

	fn handle_get(&mut self, key: K) {
		for policy_stack in self.policy_stacks.iter_mut() {
			policy_stack.update(key);
		}
	}

	fn handle_set(&mut self, key: K) {
		for policy_stack in self.policy_stacks.iter_mut() {
			policy_stack.insert(key);
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
