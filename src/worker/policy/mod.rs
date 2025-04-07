mod policy_stack;
mod event;
mod trace;

use std::{
	thread,
	sync::Arc,
	time::{Instant, Duration},
	io::{Seek, SeekFrom},
	collections::VecDeque,
};

use typesize::TypeSize;
use parking_lot::RwLock;
use crossbeam_channel::{Sender, Receiver, unbounded};
use log::info;

use kwik::{
	fmt,
	time,
};

use crate::{
	CacheSize,
	HashedKey,
	ObjectMapRef,
	StatsRef,
	OverheadManagerRef,
	EraseKey,
	erase,
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
			trace::{TraceWorker, TraceFragment},
			policy_stack::{PolicyStack, PolicyStackType},
		},
	},
};

// the sampling modulus must be a power of 2
const MINI_SAMPLING_MODULUS: u64 = 16_777_216;
const MINI_SAMPLING_THRESHOLD: u64 = 16_777;

// the polling value must be a power of 2
const RECONSTRUCT_POLICY_POLLING: usize = 1_048_576;

pub struct PolicyWorker<K, V> {
	listener: Receiver<WorkerEvent>,

	objects: ObjectMapRef<K, V>,
	stats: StatsRef,
	overhead_manager: OverheadManagerRef,

	max_cache_size: CacheSize,
	used_cache_size: CacheSize,
	policy_stack: Option<PolicyStackType>,

	trace_fragments: Arc<RwLock<VecDeque<TraceFragment>>>,
	trace_worker: Sender<StackEvent>,

	mini_policy_stacks: Box<[PolicyStackType]>,
	mini_policy_index: Option<usize>,
	current_policy: Arc<RwLock<PaperPolicy>>,

	last_set_time: Option<u64>,
}

impl<K, V> Worker for PolicyWorker<K, V>
where
	Self: 'static + Send,
	K: Eq + TypeSize,
	V: TypeSize,
{
	fn run(&mut self) -> Result<(), CacheError> {
		let (
			policy_reconstruct_tx,
			policy_reconstruct_rx,
		) = unbounded::<PolicyStackType>();

		let policy_reconstruct_tx = Arc::new(policy_reconstruct_tx);
		let mut buffered_events = Vec::<StackEvent>::new();

		loop {
			let events = self.listener
				.try_iter()
				.collect::<Vec<WorkerEvent>>();

			let mut has_current_set = false;

			for event in events {
				match event {
					WorkerEvent::Get(key, hit) if hit => self.handle_get(key),

					WorkerEvent::Set(key, size, _, old_info) => {
						self.handle_set(key, size, old_info.map(|(old_size, _)| old_size));
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

					WorkerEvent::Resize(max_cache_size) => {
						self.handle_resize(max_cache_size);
					},

					WorkerEvent::Policy(policy) => {
						self.handle_policy(policy, policy_reconstruct_tx.clone());
					},

					_ => {},
				}

				if let Some(stack_event) = StackEvent::maybe_from_worker_event(&event) {
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

impl<K, V> PolicyWorker<K, V>
where
	K: Eq + TypeSize,
	V: TypeSize,
{
	pub fn new(
		listener: WorkerReceiver,
		objects: ObjectMapRef<K, V>,
		stats: StatsRef,
		overhead_manager: OverheadManagerRef,
		policy: PaperPolicy,
	) -> Result<Self, CacheError> {
		let max_cache_size = stats.get_max_size();
		let mini_size = get_mini_stack_size(max_cache_size);

		let mini_policy_stacks = stats
			.get_policies()
			.iter()
			.map(|policy| PolicyStackType::new(*policy, mini_size))
			.collect::<Box<[_]>>();

		let policy_stack = PolicyStackType::new(policy, max_cache_size);

		let trace_fragments = Arc::new(RwLock::new(VecDeque::new()));
		let (trace_worker, trace_listener) = unbounded();

		register_worker(TraceWorker::new(
			trace_listener,
			trace_fragments.clone(),
		));

		// we need the initial size so we can accurately reconstruct the
		// policy stacks after the cache is resized
		trace_worker
			.send(StackEvent::Resize(stats.get_max_size()))
			.map_err(|_| CacheError::Internal)?;

		let worker = PolicyWorker {
			listener,

			objects,
			stats,
			overhead_manager,

			max_cache_size,
			used_cache_size: 0,

			policy_stack: Some(policy_stack),

			trace_fragments,
			trace_worker,

			mini_policy_stacks,
			mini_policy_index: None,

			current_policy: Arc::new(RwLock::new(policy)),

			last_set_time: None,
		};

		Ok(worker)
	}

	fn handle_get(&mut self, key: HashedKey) {
		if let Some(stack) = &mut self.policy_stack {
			stack.update(key);
		}

		if self.should_mini_sample(key) {
			for mini_stack in &mut self.mini_policy_stacks {
				mini_stack.update(key);
			}
		}
	}

	fn handle_set(
		&mut self,
		key: HashedKey,
		size: ObjectSize,
		old_size: Option<ObjectSize>,
	) {
		if let Some(stack) = &mut self.policy_stack {
			stack.insert(key, size);
		}

		if self.should_mini_sample(key) {
			for mini_stack in &mut self.mini_policy_stacks {
				mini_stack.insert(key, size);
			}
		}

		self.used_cache_size += size as u64;

		if let Some(old_size) = old_size {
			self.used_cache_size -= old_size as u64;
		}
	}

	fn handle_del(&mut self, key: HashedKey) {
		if let Some(stack) = &mut self.policy_stack {
			stack.remove(key);
		}

		if self.should_mini_sample(key) {
			for mini_stack in &mut self.mini_policy_stacks {
				mini_stack.remove(key);
			}
		}
	}

	fn handle_resize(&mut self, size: CacheSize) {
		self.max_cache_size = size;

		if let Some(stack) = &mut self.policy_stack {
			stack.resize(size);
		}

		let mini_size = get_mini_stack_size(size);

		for mini_stack in &mut self.mini_policy_stacks {
			mini_stack.resize(mini_size);
		}
	}

	fn handle_policy(
		&mut self,
		policy: PaperPolicy,
		policy_reconstruct_tx: Arc<Sender<PolicyStackType>>,
	) {
		if policy == *self.current_policy.read() {
			return;
		}

		info!(
			"Switching policy {} to {policy}",
			self.current_policy.read(),
		);

		*self.current_policy.write() = policy;

		let mini_policy_index = self.mini_policy_stacks
			.iter()
			.position(|mini_stack| mini_stack.is_policy(&policy))
			.unwrap_or(0);

		self.policy_stack = None;
		self.mini_policy_index = Some(mini_policy_index);

		let max_cache_size = self.max_cache_size;
		let current_policy = self.current_policy.clone();
		let trace_fragments = self.trace_fragments.clone();

		thread::spawn(move || {
			info!("Reconstructing {policy} stack");
			let now = Instant::now();

			let reconstruction_result = reconstruct_policy_stack(
				policy,
				max_cache_size,
				current_policy.clone(),
				trace_fragments.clone(),
			);

			if let Ok(stack) = reconstruction_result {
				// check to make sure the configured policy was not modified
				// before sending the reconstructed stack
				if policy == *current_policy.read() {
					info!(
						"{policy} stack reconstructed with {} object(s) in {:?}",
						fmt::number(stack.len()),
						now.elapsed(),
					);

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
		buffered_events: &[StackEvent],
		policy_reconstruct_rx: &Receiver<PolicyStackType>,
	) {
		for mut stack in policy_reconstruct_rx.try_iter() {
			for event in buffered_events {
				match event {
					StackEvent::Get(key) => stack.update(*key),
					StackEvent::Set(key, size) => stack.insert(*key, *size),
					StackEvent::Del(key) => stack.remove(*key),
					StackEvent::Wipe => stack.clear(),
					StackEvent::Resize(size) => stack.resize(*size),
				}
			}

			self.policy_stack = Some(stack);
			self.mini_policy_index = None;
		}
	}

	fn flush_buffered_events(
		&self,
		buffered_events: &mut Vec<StackEvent>,
	) -> Result<(), CacheError> {
		if self.mini_policy_index.is_some() {
			// the mini policy is still running so stack events should be buffered
			// until the full stack is reconstructed
			return Ok(());
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
		buffered_events: &mut Vec<StackEvent>,
	) -> Result<(), CacheError> {
		if let Some(index) = self.mini_policy_index {
			self.apply_mini_evictions(index, buffered_events);
			return Ok(());
		}

		let stack = self.policy_stack
			.as_mut()
			.ok_or(CacheError::Internal)?;

		while self.used_cache_size > self.max_cache_size {
			let maybe_key = stack.pop().map(|key| EraseKey::Hashed(key));

			let erase_result = erase(
				&self.objects,
				&self.stats,
				&self.overhead_manager,
				maybe_key,
			);

			let Ok((key, object)) = erase_result else {
				continue;
			};

			let size = self.overhead_manager.total_size(&object);
			self.used_cache_size -= size as u64;

			buffered_events.push(StackEvent::Del(key));
		}

		Ok(())
	}

	fn apply_mini_evictions(
		&mut self,
		mini_policy_index: usize,
		buffered_events: &mut Vec<StackEvent>,
	) {
		let mini_policy_stack = &mut self.mini_policy_stacks[mini_policy_index];
		let mut evictions = Vec::<HashedKey>::new();

		while self.used_cache_size > self.max_cache_size {
			let maybe_key = mini_policy_stack.pop().map(|key| EraseKey::Hashed(key));

			let erase_result = erase(
				&self.objects,
				&self.stats,
				&self.overhead_manager,
				maybe_key,
			);

			let Ok((key, object)) = erase_result else {
				continue;
			};

			let size = self.overhead_manager.total_size(&object);
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

	fn should_mini_sample(&self, hashed_key: HashedKey) -> bool {
		// this optimization only works if the sampling modulus is a power of 2
		hashed_key & (MINI_SAMPLING_MODULUS - 1) < MINI_SAMPLING_THRESHOLD
	}
}

fn reconstruct_policy_stack(
	policy: PaperPolicy,
	max_size: CacheSize,
	current_policy: Arc<RwLock<PaperPolicy>>,
	trace_fragments: Arc<RwLock<VecDeque<TraceFragment>>>,
) -> Result<PolicyStackType, CacheError> {
	let mut stack = PolicyStackType::new(policy, max_size);

	for fragment in trace_fragments.read().iter() {
		let mut fragment_modifiers = fragment.lock();
		let fragment_reader = &mut fragment_modifiers.0;

		let initial_position = fragment_reader
			.stream_position()
			.map_err(|_| CacheError::Internal)?;

		// start reading the file from the beginning
		fragment_reader
			.rewind()
			.map_err(|_| CacheError::Internal)?;

		for (index, event) in fragment_reader.iter().enumerate() {
			if index & (RECONSTRUCT_POLICY_POLLING - 1) == 0 && policy != *current_policy.read() {
				// every RECONSTRUCT_POLICY_POLLING events, check if the currently
				// configured policy is still the policy we're reconstructing and
				// if it's not, move the reader back to its original position in
				// the file and terminate the reconstruction
				fragment_reader
					.seek(SeekFrom::Start(initial_position))
					.map_err(|_| CacheError::Internal)?;

				return Err(CacheError::Internal);
			}

			match event {
				StackEvent::Get(key) => stack.update(key),
				StackEvent::Set(key, size) => stack.insert(key, size),
				StackEvent::Del(key) => stack.remove(key),
				StackEvent::Wipe => stack.clear(),
				StackEvent::Resize(size) => stack.resize(size),
			}
		}

		// ensure the underlying trace fragment is returned back to its original
		// position (this is mostly just a sanity check as reading the file should
		// already return it to the end which should be the orignal position)
		fragment_reader
			.seek(SeekFrom::Start(initial_position))
			.map_err(|_| CacheError::Internal)?;
	}

	Ok(stack)
}

fn get_mini_stack_size(size: CacheSize) -> CacheSize {
	let ratio = MINI_SAMPLING_THRESHOLD as f64 / MINI_SAMPLING_MODULUS as f64;
	(size as f64 * ratio) as u64
}

unsafe impl<K, V> Send for PolicyWorker<K, V>
where
	K: TypeSize,
	V: TypeSize,
{}
