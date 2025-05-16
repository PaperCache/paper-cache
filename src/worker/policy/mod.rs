mod policy_stack;
mod mini_stack;
mod event;
mod trace;

use std::{
	thread,
	cmp::Ordering,
	sync::Arc,
	time::{Instant, Duration},
	io::{Seek, SeekFrom},
	collections::VecDeque,
};

use rayon::prelude::*;
use typesize::TypeSize;
use parking_lot::RwLock;
use crossbeam_channel::{Sender, Receiver, unbounded};
use log::info;
use kwik::fmt;

use crate::{
	CacheSize,
	HashedKey,
	ObjectMapRef,
	StatsRef,
	OverheadManagerRef,
	EraseKey,
	erase,
	error::CacheError,
	policy::PaperPolicy,
	object::{
		ObjectSize,
		overhead::get_policy_overhead,
	},
	worker::{
		Worker,
		WorkerEvent,
		WorkerReceiver,
		register_worker,
		policy::{
			mini_stack::MiniStack,
			event::StackEvent,
			trace::{TraceWorker, TraceFragment},
			policy_stack::{PolicyStack, init_policy_stack},
		},
	},
};

// the sampling modulus must be a power of 2
const MINI_SAMPLING_MODULUS: u64 = 16_777_216;
const MINI_SAMPLING_THRESHOLD: u64 = 16_777;

// the polling value must be a power of 2
const RECONSTRUCT_POLICY_POLLING: usize = 1_048_576;

const AUTO_POLICY_DURATION: Duration = Duration::from_secs(3_600);
const SET_RECENCY_DURATION: Duration = Duration::from_secs(5);
const SHORT_POLLING_DURATION: Duration = Duration::from_millis(1);
const LONG_POLLING_DURATION: Duration = Duration::from_secs(1);

pub struct PolicyWorker<K, V> {
	listener: Receiver<WorkerEvent>,

	objects: ObjectMapRef<K, V>,
	stats: StatsRef,
	overhead_manager: OverheadManagerRef,

	policy_stack: Option<Box<dyn PolicyStack>>,

	trace_fragments: Arc<RwLock<VecDeque<TraceFragment>>>,
	trace_worker: Sender<StackEvent>,

	mini_stacks: Box<[MiniStack]>,
	mini_index: Option<usize>,
	current_policy: Arc<RwLock<PaperPolicy>>,

	last_auto_policy_time: Option<Instant>,
	last_set_time: Option<Instant>,
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
		) = unbounded::<Box<dyn PolicyStack>>();

		let policy_reconstruct_tx = Arc::new(policy_reconstruct_tx);
		let mut buffered_events = Vec::<StackEvent>::new();

		loop {
			let events = self.listener
				.try_iter()
				.collect::<Vec<WorkerEvent>>();

			let mut has_current_set = false;

			for event in events {
				match event {
					WorkerEvent::Get(key, _) => self.handle_get(key),

					WorkerEvent::Set(key, size, _, _) => {
						self.handle_set(key, size);
						has_current_set = true;
					},

					WorkerEvent::Del(key, _) => self.handle_del(key),
					WorkerEvent::Wipe => self.handle_wipe(),
					WorkerEvent::Resize(max_size) => self.handle_resize(max_size),

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

			let now = Instant::now();

			if let Some(policy) = self.perform_auto_policy(now, has_current_set) {
				self.stats.set_auto_policy(policy)?;
				self.handle_policy(policy, policy_reconstruct_tx.clone());
			}

			self.delay_event_loop(now, has_current_set);
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
	) -> Result<Self, CacheError> {
		let max_cache_size = stats.get_max_size();
		let mini_size = get_mini_stack_size(max_cache_size);

		let mini_stacks = stats
			.get_policies()
			.iter()
			.map(|policy| MiniStack::new(*policy, mini_size))
			.collect::<Box<[_]>>();

		let policy = stats.get_policy();
		let policy_stack = init_policy_stack(policy, max_cache_size);

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

			policy_stack: Some(policy_stack),

			trace_fragments,
			trace_worker,

			mini_stacks,
			mini_index: None,

			current_policy: Arc::new(RwLock::new(policy)),

			last_auto_policy_time: None,
			last_set_time: None,
		};

		Ok(worker)
	}

	fn handle_get(&mut self, key: HashedKey) {
		if let Some(stack) = &mut self.policy_stack {
			stack.update(key);
		}

		if self.should_mini_sample(key) {
			self.mini_stacks
				.par_iter_mut()
				.for_each(|mini_stack| mini_stack.update_with_count(key));
		}
	}

	fn handle_set(&mut self, key: HashedKey, size: ObjectSize) {
		if let Some(stack) = &mut self.policy_stack {
			stack.insert(key, size);
		}

		if self.should_mini_sample(key) {
			self.mini_stacks
				.par_iter_mut()
				.for_each(|mini_stack| mini_stack.insert(key, size));
		}
	}

	fn handle_del(&mut self, key: HashedKey) {
		if let Some(stack) = &mut self.policy_stack {
			stack.remove(key);
		}

		if self.should_mini_sample(key) {
			self.mini_stacks
				.par_iter_mut()
				.for_each(|mini_stack| mini_stack.remove(key));
		}
	}

	fn handle_resize(&mut self, size: CacheSize) {
		if let Some(stack) = &mut self.policy_stack {
			stack.resize(size);
		}

		let mini_size = get_mini_stack_size(size);

		self.mini_stacks
			.par_iter_mut()
			.for_each(|mini_stack| mini_stack.resize(mini_size));
	}

	fn handle_policy(
		&mut self,
		policy: PaperPolicy,
		policy_reconstruct_tx: Arc<Sender<Box<dyn PolicyStack>>>,
	) {
		if policy.is_auto() || policy == *self.current_policy.read() {
			return;
		}

		info!(
			"Switching policy {} to {policy}",
			self.current_policy.read(),
		);

		*self.current_policy.write() = policy;

		let mini_index = self.mini_stacks
			.iter()
			.position(|mini_stack| mini_stack.is_policy(&policy))
			.unwrap_or(0);

		self.policy_stack = None;
		self.mini_index = Some(mini_index);

		let max_cache_size = self.stats.get_max_size();
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

		self.mini_stacks
			.par_iter_mut()
			.for_each(|mini_stack| mini_stack.clear());
	}

	fn apply_buffered_events(
		&mut self,
		buffered_events: &[StackEvent],
		policy_reconstruct_rx: &Receiver<Box<dyn PolicyStack>>,
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

			info!("Policy switch complete");

			self.policy_stack = Some(stack);
			self.mini_index = None;
		}
	}

	fn flush_buffered_events(
		&self,
		buffered_events: &mut Vec<StackEvent>,
	) -> Result<(), CacheError> {
		if self.mini_index.is_some() {
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
		if let Some(index) = self.mini_index {
			self.apply_mini_evictions(index, buffered_events);
			return Ok(());
		}

		let policy = self.current_policy.read();
		let max_cache_size = self.stats.get_max_size();

		while self.stats.get_used_size(&policy) > max_cache_size {
			let maybe_key = self.policy_stack
				.as_mut()
				.ok_or(CacheError::Internal)?
				.pop().map(|key| EraseKey::Hashed(key));

			let erase_result = erase(
				&self.objects,
				&self.stats,
				&self.overhead_manager,
				maybe_key,
			);

			let Ok((key, _)) = erase_result else {
				continue;
			};

			buffered_events.push(StackEvent::Del(key));
		}

		Ok(())
	}

	fn apply_mini_evictions(
		&mut self,
		mini_index: usize,
		buffered_events: &mut Vec<StackEvent>,
	) {
		let max_cache_size = self.stats.get_max_size();
		let policy = self.current_policy.read();
		let mut evictions = Vec::<HashedKey>::new();

		while self.stats.get_used_size(&policy) > max_cache_size {
			let maybe_key = self.mini_stacks[mini_index]
				.pop().map(|key| EraseKey::Hashed(key));

			let erase_result = erase(
				&self.objects,
				&self.stats,
				&self.overhead_manager,
				maybe_key,
			);

			let Ok((key, _)) = erase_result else {
				continue;
			};

			evictions.push(key);
			buffered_events.push(StackEvent::Del(key));
		}

		for key in &evictions {
			self.mini_stacks
				.par_iter_mut()
				.enumerate()
				.filter(|(index, _)| *index != mini_index)
				.for_each(|(_, mini_stack)| mini_stack.remove(*key));
		}
	}

	fn should_mini_sample(&self, hashed_key: HashedKey) -> bool {
		// this optimization only works if the sampling modulus is a power of 2
		hashed_key & (MINI_SAMPLING_MODULUS - 1) < MINI_SAMPLING_THRESHOLD
	}

	fn perform_auto_policy(&mut self, now: Instant, has_current_set: bool) -> Option<PaperPolicy> {
		if has_current_set || !self.stats.is_auto_policy() || self.mini_index.is_some() {
			// don't switch the policy while (any of):
			// * there is recent set activity
			// * the auto policy is not configured
			// * a stack is being reconstructed
			return None;
		}

		let should_poll_policy = self.last_auto_policy_time
			.is_none_or(|last_auto_policy_time| now - last_auto_policy_time > AUTO_POLICY_DURATION);

		if !should_poll_policy {
			return None;
		}

		self.last_auto_policy_time = Some(now);
		self.get_optimal_policy()
	}

	fn get_optimal_policy(&self) -> Option<PaperPolicy> {
		let current_miss_ratio = self.mini_stacks
			.iter()
			.find_map(|mini_stack| {
				if !mini_stack.is_policy(&self.current_policy.read()) {
					return None;
				}

				Some(mini_stack.miss_ratio())
			})?;

		let optimal_mini_stack = self.mini_stacks
			.iter()
			.min_by(|a, b| {
				match a.miss_ratio().total_cmp(&b.miss_ratio()) {
					Ordering::Equal => {
						// the two mini stacks have the same miss ratios, so
						// select the one with the lower memory overhead
						let a_overhead = get_policy_overhead(&a.policy());
						let b_overhead = get_policy_overhead(&b.policy());

						a_overhead.cmp(&b_overhead)
					},

					cmp => cmp,
				}
			})?;

		if optimal_mini_stack.miss_ratio() < current_miss_ratio {
			// make sure we only switch to a different policy that performs better
			// than the current policy
			Some(optimal_mini_stack.policy())
		} else {
			None
		}
	}

	fn delay_event_loop(&mut self, now: Instant, has_current_set: bool) {
		let has_recent_set = self.last_set_time
			.is_some_and(|last_set_time| now - last_set_time <= SET_RECENCY_DURATION);

		if has_current_set {
			self.last_set_time = Some(now);
		}

		let delay = if has_recent_set {
			SHORT_POLLING_DURATION
		} else {
			LONG_POLLING_DURATION
		};

		thread::sleep(delay);
	}
}

fn reconstruct_policy_stack(
	policy: PaperPolicy,
	max_size: CacheSize,
	current_policy: Arc<RwLock<PaperPolicy>>,
	trace_fragments: Arc<RwLock<VecDeque<TraceFragment>>>,
) -> Result<Box<dyn PolicyStack>, CacheError> {
	let mut stack = init_policy_stack(policy, max_size);

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
