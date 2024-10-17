mod policy_stack;
mod event;
mod trace;

use std::{
	thread,
	sync::Arc,
	hash::{Hash, BuildHasher},
	time::{Instant, Duration},
	io::Seek,
	fs::File,
	collections::VecDeque,
};

use typesize::TypeSize;
use parking_lot::RwLock;
use crossbeam_channel::{Sender, Receiver, unbounded};

use kwik::{
	time,
	file::{
		FileReader,
		binary::{BinaryReader, ReadChunk, WriteChunk},
	},
};

use crate::{
	cache::{
		CacheSize,
		ObjectMapRef,
		StatsRef,
		OverheadManagerRef,
		POLICIES,
		erase,
	},
	object::ObjectSize,
	error::CacheError,
	policy::PaperPolicy,
	worker::{
		Worker,
		WorkerEvent,
		WorkerReceiver,
		register_worker,
		policy::{
			event::StackEvent,
			trace::TraceWorker,
			policy_stack::{PolicyStack, PolicyStackType},
		},
	},
};

// the sampling modulus must be a power of 2
const MINI_SAMPLING_MODULUS: u64 = 16777216;
const MINI_SAMPLING_THRESHOLD: u64 = 16777;

pub struct PolicyWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Clone + Send + BuildHasher,
{
	listener: Receiver<WorkerEvent<K>>,

	objects: ObjectMapRef<K, V, S>,
	stats: StatsRef,
	overhead_manager: OverheadManagerRef,

	max_cache_size: CacheSize,
	used_cache_size: CacheSize,
	policy_stack: Option<PolicyStackType<K, S>>,

	traces: Arc<RwLock<VecDeque<(Instant, File)>>>,
	trace_worker: Sender<StackEvent<K>>,

	mini_policy_stacks: Box<[PolicyStackType<K, S>]>,
	mini_policy_index: Option<usize>,
	current_policy: Arc<RwLock<PaperPolicy>>,

	last_set_time: Option<u64>,

	mini_stack_hasher: S,
	hasher: S,
}

impl<K, V, S> Worker<K, V, S> for PolicyWorker<K, V, S>
where
	Self: 'static + Send,
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Clone + Send + BuildHasher,
{
	fn run(&mut self) -> Result<(), CacheError> {
		let (
			policy_reconstruct_tx,
			policy_reconstruct_rx,
		) = unbounded::<PolicyStackType<K, S>>();

		let policy_reconstruct_tx = Arc::new(policy_reconstruct_tx);
		let mut buffered_events = Vec::<StackEvent<K>>::new();

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
						self.used_cache_size -= size as u64;
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

				if let Some(stack_event) = StackEvent::<K>::maybe_from_worker_event(&event) {
					if self.policy_stack.is_some() {
						self.trace_worker
							.send(stack_event)
							.map_err(|_| CacheError::Internal)?;
					} else {
						buffered_events.push(stack_event);
					}
				}
			}

			self.apply_buffered_events(&buffered_events, &policy_reconstruct_rx);
			self.flush_buffered_events(&mut buffered_events)?;
			self.apply_evictions(&mut buffered_events)?;

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
	S: 'static + Clone + Send + BuildHasher,
{
	pub fn with_hasher(
		listener: WorkerReceiver<K>,
		objects: ObjectMapRef<K, V, S>,
		stats: StatsRef,
		overhead_manager: OverheadManagerRef,
		policy: PaperPolicy,
		hasher: S,
	) -> Self {
		let max_cache_size = stats.get_max_size();

		let mini_policy_stacks = POLICIES
			.iter()
			.map(|policy| PolicyStackType::<K, S>::init_with_hasher(*policy, hasher.clone()))
			.collect::<Box<[_]>>();

		let policy_stack = PolicyStackType::<K, S>::init_with_hasher(
			policy,
			hasher.clone(),
		);

		let traces = Arc::new(RwLock::new(VecDeque::new()));
		let (trace_worker, trace_listener) = unbounded();

		register_worker(TraceWorker::<K, V, S>::new(
			trace_listener,
			traces.clone(),
		));

		PolicyWorker {
			listener,

			objects,
			stats,
			overhead_manager,

			max_cache_size,
			used_cache_size: 0,

			policy_stack: Some(policy_stack),

			traces,
			trace_worker,

			mini_policy_stacks,
			mini_policy_index: None,

			current_policy: Arc::new(RwLock::new(policy)),

			last_set_time: None,

			mini_stack_hasher: hasher.clone(),
			hasher,
		}
	}

	fn handle_get(&mut self, key: K) {
		if let Some(stack) = &mut self.policy_stack {
			stack.update(key);
		}

		if self.should_mini_sample(key) {
			for mini_stack in &mut self.mini_policy_stacks {
				mini_stack.update(key);
			}
		}
	}

	fn handle_set(&mut self, key: K, size: ObjectSize, old_size: Option<ObjectSize>) {
		if let Some(stack) = &mut self.policy_stack {
			stack.insert(key);
		}

		if self.should_mini_sample(key) {
			for mini_stack in &mut self.mini_policy_stacks {
				mini_stack.insert(key);
			}
		}

		self.used_cache_size += size as u64;

		if let Some(old_size) = old_size {
			self.used_cache_size -= old_size as u64;
		}
	}

	fn handle_del(&mut self, key: K) {
		if let Some(stack) = &mut self.policy_stack {
			stack.remove(key);
		}

		if self.should_mini_sample(key) {
			for mini_stack in &mut self.mini_policy_stacks {
				mini_stack.remove(key);
			}
		}
	}

	fn handle_policy(
		&mut self,
		policy: PaperPolicy,
		policy_reconstruct_tx: Arc<Sender<PolicyStackType<K, S>>>,
	) {
		if policy == *self.current_policy.read() {
			return;
		}

		*self.current_policy.write() = policy;

		let mini_policy_index = self.mini_policy_stacks
			.iter()
			.position(|mini_stack| mini_stack.is_policy(policy))
			.unwrap_or(0);

		self.policy_stack = None;
		self.mini_policy_index = Some(mini_policy_index);

		let current_policy = self.current_policy.clone();
		let traces = self.traces.clone();
		let hasher = self.hasher.clone();

		thread::spawn(move || {
			let reconstruction_result = reconstruct_policy_stack::<K, S>(
				policy,
				current_policy.clone(),
				traces.clone(),
				hasher,
			);

			if let Ok(stack) = reconstruction_result {
				// check to make sure the configured policy was not modified
				// before sending the reconstructed stack
				if policy == *current_policy.read() {
					let _ = policy_reconstruct_tx.send(stack);
				}
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
		buffered_events: &[StackEvent<K>],
		policy_reconstruct_rx: &Receiver<PolicyStackType<K, S>>,
	) {
		for mut stack in policy_reconstruct_rx.try_iter() {
			for event in buffered_events {
				match event {
					StackEvent::Get(key) => stack.update(*key),
					StackEvent::Set(key) => stack.insert(*key),
					StackEvent::Del(key) => stack.remove(*key),
					StackEvent::Wipe => stack.clear(),
				}
			}

			self.policy_stack = Some(stack);
			self.mini_policy_index = None;
		}
	}

	fn flush_buffered_events(
		&self,
		buffered_events: &mut Vec<StackEvent<K>>,
	) -> Result<(), CacheError> {
		if self.mini_policy_index.is_some() {
			// the mini policy is still running so stack events should be buffered
			// until the full stack is reconstructed
		}

		for event in buffered_events.iter() {
			self.trace_worker
				.send(event.clone())
				.map_err(|_| CacheError::Internal)?;
		}

		buffered_events.clear();

		Ok(())
	}

	fn apply_evictions(
		&mut self,
		buffered_events: &mut Vec<StackEvent<K>>,
	) -> Result<(), CacheError> {
		if let Some(index) = self.mini_policy_index {
			self.apply_mini_evictions(index, buffered_events);
			return Ok(());
		}

		let stack = self.policy_stack
			.as_mut()
			.ok_or(CacheError::Internal)?;

		while self.used_cache_size > self.max_cache_size {
			let erase_result = erase(
				&self.objects,
				&self.stats,
				&self.overhead_manager,
				stack.pop(),
			);

			let Ok((key, object)) = erase_result else {
				continue;
			};

			let size = self.overhead_manager.total_size(key, &object);
			self.used_cache_size -= size as u64;

			buffered_events.push(StackEvent::Del(key));
		}

		Ok(())
	}

	fn apply_mini_evictions(
		&mut self,
		mini_policy_index: usize,
		buffered_events: &mut Vec<StackEvent<K>>,
	) {
		let mini_policy_stack = &mut self.mini_policy_stacks[mini_policy_index];
		let mut evictions = Vec::<K>::new();

		while self.used_cache_size > self.max_cache_size {
			let erase_result = erase(
				&self.objects,
				&self.stats,
				&self.overhead_manager,
				mini_policy_stack.pop(),
			);

			let Ok((key, object)) = erase_result else {
				continue;
			};

			let size = self.overhead_manager.total_size(key, &object);
			self.used_cache_size -= size as u64;

			evictions.push(key);
			buffered_events.push(StackEvent::Del(key));
		}

		for key in &evictions {
			for (index, mini_stack) in self.mini_policy_stacks.iter_mut().enumerate() {
				if index != mini_policy_index {
					mini_stack.remove(*key);
				}
			}
		}
	}

	fn should_mini_sample(&self, key: K) -> bool {
		let hashed = self.mini_stack_hasher.hash_one(key);

		// this optimization only works if the sampling modulus is a power of 2
		hashed & (MINI_SAMPLING_MODULUS - 1) < MINI_SAMPLING_THRESHOLD
	}
}

fn reconstruct_policy_stack<K, S>(
	policy: PaperPolicy,
	current_policy: Arc<RwLock<PaperPolicy>>,
	traces: Arc<RwLock<VecDeque<(Instant, File)>>>,
	hasher: S,
) -> Result<PolicyStackType<K, S>, CacheError>
where
	K: 'static + Copy + Eq + Hash + Sync + TypeSize + ReadChunk + WriteChunk,
	S: Clone + Send + BuildHasher,
{
	let mut stack = PolicyStackType::<K, S>::init_with_hasher(policy, hasher);

	for (_, file) in traces.read().iter() {
		let mut file = file
			.try_clone()
			.map_err(|_| CacheError::Internal)?;

		let initial_position = file
			.stream_position()
			.map_err(|_| CacheError::Internal)?;

		// start reading the file from the beginning
		file.rewind().map_err(|_| CacheError::Internal)?;

		let mut reader = BinaryReader::<StackEvent<K>>::from_file(file)
			.map_err(|_| CacheError::Internal)?;

		for (index, event) in reader.iter().enumerate() {
			if index % 1_000_000 == 0 && policy != *current_policy.read() {
				// every 1_000_000 events, check if the currently configured policy
				// is still the policy we're reconstructing and if it's not, move the
				// reader back to its original position in the file and terminate
				// the reconstruction
				reader
					.offset(initial_position)
					.map_err(|_| CacheError::Internal)?;

				return Err(CacheError::Internal);
			}

			match event {
				StackEvent::Get(key) => stack.update(key),
				StackEvent::Set(key) => stack.insert(key),
				StackEvent::Del(key) => stack.remove(key),
				StackEvent::Wipe => stack.clear(),
			}
		}

		// ensure the underlying file is returned back to its original position
		// (this is mostly just a sanity check as reading the file should
		// already return it to the end which should be the orignal position)
		reader
			.offset(initial_position)
			.map_err(|_| CacheError::Internal)?;
	}

	Ok(stack)
}

unsafe impl<K, V, S> Send for PolicyWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Clone + Send + BuildHasher,
{}
