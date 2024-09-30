use std::{
	thread,
	sync::{Arc, RwLock},
	hash::{Hash, BuildHasher},
	time::{Instant, Duration},
	io::{Seek, SeekFrom},
	fs::File,
	collections::{
		VecDeque,
		hash_map::{HashMap, Entry},
	},
};

use typesize::TypeSize;
use crossbeam_channel::{Sender, Receiver, unbounded};

use kwik::{
	time,
	file::{
		FileReader,
		binary::{BinaryReader, ReadChunk, WriteChunk},
	},
};

use crate::{
	cache::{CacheSize, ObjectMapRef, StatsRef, OverheadManagerRef, erase},
	object::ObjectSize,
	error::CacheError,
	worker::{
		Worker,
		WorkerEvent,
		WorkerReceiver,
		trace::Access,
	},
	policy::{
		PaperPolicy,
		PolicyStack,
		PolicyStackType,
		MiniStackType,
	},
};

pub struct PolicyWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{
	listener: Receiver<WorkerEvent<K>>,

	objects: ObjectMapRef<K, V, S>,
	stats: StatsRef,
	overhead_manager: OverheadManagerRef,

	max_cache_size: CacheSize,
	used_cache_size: CacheSize,
	policy_stack: Option<PolicyStackType<K>>,
	traces: Arc<RwLock<VecDeque<(Instant, File)>>>,

	mini_policy_stacks: Box<[MiniStackType<K>]>,
	mini_policy_index: Option<usize>,

	last_set_time: Option<u64>,
}

impl<K, V, S> Worker<K, V, S> for PolicyWorker<K, V, S>
where
	Self: 'static + Send,
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{
	fn run(&mut self) -> Result<(), CacheError> {
		let (
			policy_reconstruct_tx,
			policy_reconstruct_rx,
		) = unbounded::<PolicyStackType<K>>();

		let policy_reconstruct_tx = Arc::new(policy_reconstruct_tx);
		let mut buffered_events = Vec::<WorkerEvent<K>>::new();

		loop {
			let events = self.listener
				.try_iter()
				.collect::<Vec<WorkerEvent<K>>>();

			let mut has_current_set = false;

			for event in events {
				match event {
					WorkerEvent::Get(key, hit) if hit => self.handle_get(key),

					WorkerEvent::Set(key, size, _, old_size) => {
						self.handle_set(key, size, old_size);
						has_current_set = true;
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
						self.handle_policy(policy, policy_reconstruct_tx.clone());
					},

					_ => {},
				}

				if self.mini_policy_index.is_some() {
					buffered_events.push(event);
				}
			}

			self.apply_buffered_events(&mut buffered_events, &policy_reconstruct_rx);
			self.apply_evictions(&mut buffered_events);

			let now = time::timestamp();

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
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{
	pub fn new(
		listener: WorkerReceiver<K>,
		objects: ObjectMapRef<K, V, S>,
		stats: StatsRef,
		overhead_manager: OverheadManagerRef,
		policy: PaperPolicy,
		traces: Arc<RwLock<VecDeque<(Instant, File)>>>,
	) -> Self {
		let max_cache_size = stats.get_max_size();

		let mini_policy_stacks = vec![
			PaperPolicy::Lfu.into(),
			PaperPolicy::Fifo.into(),
			PaperPolicy::Lru.into(),
			PaperPolicy::Mru.into(),
		].into_boxed_slice();

		PolicyWorker {
			listener,

			objects,
			stats,
			overhead_manager,

			max_cache_size,
			used_cache_size: 0,

			policy_stack: Some(policy.into()),
			traces,

			mini_policy_stacks,
			mini_policy_index: None,

			last_set_time: None,
		}
	}

	fn handle_get(&mut self, key: K) {
		if let Some(stack) = &mut self.policy_stack {
			stack.update(key);
		}

		for mini_stack in &mut self.mini_policy_stacks {
			mini_stack.update(key);
		}
	}

	fn handle_set(&mut self, key: K, size: ObjectSize, old_size: Option<ObjectSize>) {
		if let Some(stack) = &mut self.policy_stack {
			stack.insert(key);
		}

		for mini_stack in &mut self.mini_policy_stacks {
			mini_stack.insert(key);
		}

		self.used_cache_size += size;

		if let Some(old_size) = old_size {
			self.used_cache_size -= old_size;
		}
	}

	fn handle_del(&mut self, key: K) {
		if let Some(stack) = &mut self.policy_stack {
			stack.remove(key);
		}

		for mini_stack in &mut self.mini_policy_stacks {
			mini_stack.remove(key);
		}
	}

	fn handle_policy(
		&mut self,
		policy: PaperPolicy,
		policy_reconstruct_tx: Arc<Sender<PolicyStackType<K>>>,
	) {
		let is_current_policy = self.policy_stack
			.as_ref()
			.is_some_and(|policy_stack| policy_stack.is_policy(policy));

		if is_current_policy {
			return;
		}

		let mini_policy_index = self.mini_policy_stacks
			.iter()
			.position(|mini_stack| mini_stack.is_policy(policy))
			.unwrap_or(0);

		self.policy_stack = None;
		self.mini_policy_index = Some(mini_policy_index);

		let max_cache_size = self.max_cache_size;
		let traces = self.traces.clone();

		thread::spawn(move || {
			let reconstruction_result = reconstruct_policy_stack::<K>(
				policy,
				max_cache_size,
				traces.clone(),
			);

			if let Ok(stack) = reconstruction_result {
				let _ = policy_reconstruct_tx.send(stack);
			}
		});
	}

	fn handle_wipe(&mut self) {
		if let Some(stack) = &mut self.policy_stack {
			stack.clear();
		}

		for mini_stack in &mut self.mini_policy_stacks {
			mini_stack.clear();
		}
	}

	fn apply_buffered_events(
		&mut self,
		buffered_events: &mut Vec<WorkerEvent<K>>,
		policy_reconstruct_rx: &Receiver<PolicyStackType<K>>,
	) {
		for mut reconstructed_stack in policy_reconstruct_rx.try_iter() {
			for event in buffered_events.iter() {
				match event {
					WorkerEvent::Get(key, hit) if *hit => reconstructed_stack.update(*key),
					WorkerEvent::Set(key, _, _, _) => reconstructed_stack.insert(*key),
					WorkerEvent::Del(key, _, _) => reconstructed_stack.remove(*key),
					WorkerEvent::Wipe => reconstructed_stack.clear(),

					_ => {},
				}
			}

			buffered_events.clear();

			self.policy_stack = Some(reconstructed_stack);
			self.mini_policy_index = None;
		}
	}

	fn apply_evictions(&mut self, buffered_events: &mut Vec<WorkerEvent<K>>) {
		if let Some(index) = self.mini_policy_index {
			self.apply_mini_evictions(index, buffered_events);
		}

		if let Some(stack) = self.policy_stack.as_mut() {
			while self.used_cache_size > self.max_cache_size {
				let Some(key) = stack.pop() else {
					continue;
				};

				let Ok(object) = erase(&self.objects, &self.stats, &self.overhead_manager, key) else {
					continue;
				};

				self.used_cache_size -= self.overhead_manager.total_size(key, &object);
			}
		}
	}

	fn apply_mini_evictions(
		&mut self,
		mini_policy_index: usize,
		buffered_events: &mut Vec<WorkerEvent<K>>,
	) {
		let mini_policy_stack = &mut self.mini_policy_stacks[mini_policy_index];
		let mut evictions = Vec::<K>::new();

		while self.used_cache_size > self.max_cache_size {
			let Some(key) = mini_policy_stack.pop() else {
				// the mini stack is empty, but it's okay because the cache will just use
				// a little more memory until the policy stack is reconstructed, so we want
				// to be sure we don't get stuck here

				break;
			};

			let Ok(object) = erase(&self.objects, &self.stats, &self.overhead_manager, key) else {
				continue;
			};

			let size = self.overhead_manager.total_size(key, &object);
			self.used_cache_size -= size;

			evictions.push(key);
			buffered_events.push(WorkerEvent::Del(key, size, None));
		}

		for key in &evictions {
			for (index, mini_stack) in self.mini_policy_stacks.iter_mut().enumerate() {
				if index != mini_policy_index {
					mini_stack.remove(*key);
				}
			}
		}
	}
}

fn reconstruct_policy_stack<K>(
	policy: PaperPolicy,
	max_size: CacheSize,
	traces: Arc<RwLock<VecDeque<(Instant, File)>>>,
) -> Result<PolicyStackType<K>, CacheError>
where
	K: 'static + Copy + Eq + Hash + Sync + TypeSize + ReadChunk + WriteChunk,
{
	let mut stack: PolicyStackType<K> = policy.into();
	let mut size_map = HashMap::<K, ObjectSize>::new();

	let mut current_size = 0u64;

	for (_, file) in traces.read().map_err(|_| CacheError::Internal)?.iter() {
		let mut file = file.try_clone().map_err(|_| CacheError::Internal)?;
		file.seek(SeekFrom::Start(0)).map_err(|_| CacheError::Internal)?;

		let reader = BinaryReader::<Access<K>>::from_file(file)
			.map_err(|_| CacheError::Internal)?;

		for access in reader {
			match size_map.entry(access.key()) {
				Entry::Occupied(o) => {
					stack.update(access.key());

					let saved_size = o.into_mut();
					current_size -= *saved_size;
					*saved_size = access.size();
				},

				Entry::Vacant(v) => {
					stack.insert(access.key());
					v.insert(access.size());
				},
			};

			current_size += access.size();

			while current_size > max_size {
				if let Some(size) = stack.pop().and_then(|key| size_map.remove(&key)) {
					current_size -= size;
				}
			}
		}
	}

	Ok(stack)
}

unsafe impl<K, V, S> Send for PolicyWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{}
