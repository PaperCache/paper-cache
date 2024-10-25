mod fragment;

use std::{
	thread,
	sync::Arc,
	hash::{Hash, BuildHasher},
	time::Duration,
	collections::VecDeque,
	marker::PhantomData,
};

use typesize::TypeSize;
use parking_lot::RwLock;
use crossbeam_channel::Receiver;

use kwik::file::{
	FileWriter,
	binary::{ReadChunk, WriteChunk},
};

use crate::{
	error::CacheError,
	worker::{
		Worker,
		policy::event::StackEvent,
	},
};

pub use crate::worker::policy::trace::fragment::TraceFragment;

const POLL_DELAY: Duration = Duration::from_secs(1);

pub struct TraceWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Clone + BuildHasher,
{
	listener: Receiver<StackEvent<K>>,

	trace_fragments: Arc<RwLock<VecDeque<TraceFragment<K>>>>,

	_v_marker: PhantomData<V>,
	_s_marker: PhantomData<S>,
}

impl<K, V, S> Worker<K, V, S> for TraceWorker<K, V, S>
where
	Self: 'static + Send,
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Clone + BuildHasher,
{
	fn run(&mut self) -> Result<(), CacheError> {
		loop {
			let events = self.listener
				.try_iter()
				.collect::<Vec<_>>();

			if !events.is_empty() {
				self.refresh_fragments()?;

				let fragments = self.trace_fragments.read();
				let fragment = fragments.back().ok_or(CacheError::Internal)?;

				let mut modifiers = fragment.lock();
				let writer = &mut modifiers.1;

				for event in events {
					writer
						.write_chunk(&event)
						.map_err(|_| CacheError::Internal)?;
				}

				writer
					.flush()
					.map_err(|_| CacheError::Internal)?;
			}

			thread::sleep(POLL_DELAY);
		}
	}
}

impl<K, V, S> TraceWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Clone + BuildHasher,
{
	pub fn new(
		listener: Receiver<StackEvent<K>>,
		trace_fragments: Arc<RwLock<VecDeque<TraceFragment<K>>>>,
	) -> Self {
		TraceWorker {
			listener,

			trace_fragments,

			_v_marker: PhantomData,
			_s_marker: PhantomData,
		}
	}

	/// Ensures all trace fragments are younger than TRACE_MAX_AGE and the
	/// youngest fragment is also younger than TRACE_REFRESH_AGE
	fn refresh_fragments(&mut self) -> Result<(), CacheError> {
		// remove any fragments that are expired
		while self.trace_fragments
			.read()
			.front()
			.is_some_and(|fragment| fragment.is_expired()) {

			self.trace_fragments.write().pop_front();
		}

		if self.trace_fragments
			.read()
			.back()
			.is_some_and(|fragment| fragment.is_valid()) {

			// the latest trace is still valid
			return Ok(());
		}

		// the latest fragment is no longer valid, so create a new one
		let fragment = TraceFragment::<K>::new()
			.map_err(|_| CacheError::Internal)?;

		self.trace_fragments
			.write()
			.push_back(fragment);

		Ok(())
	}
}

unsafe impl<K, V, S> Send for TraceWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Clone + BuildHasher,
{}
