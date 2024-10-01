use std::{
	thread,
	sync::Arc,
	hash::{Hash, BuildHasher},
	time::{Instant, Duration},
	fs::File,
	collections::VecDeque,
	marker::PhantomData,
};

use typesize::TypeSize;
use parking_lot::RwLock;
use crossbeam_channel::Receiver;
use tempfile::tempfile;

use kwik::file::{
	FileWriter,
	binary::{BinaryWriter, ReadChunk, WriteChunk},
};

use crate::{
	error::CacheError,
	worker::{
		Worker,
		policy::event::StackEvent,
	},
};

const TRACE_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const TRACE_REFRESH_AGE: Duration = Duration::from_secs(60 * 60);
const POLL_DELAY: Duration = Duration::from_secs(1);

pub struct TraceWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{
	listener: Receiver<StackEvent<K>>,

	traces: Arc<RwLock<VecDeque<(Instant, File)>>>,
	current_writer: Option<BinaryWriter<StackEvent<K>>>,

	_v_marker: PhantomData<V>,
	_s_marker: PhantomData<S>,
}

impl<K, V, S> Worker<K, V, S> for TraceWorker<K, V, S>
where
	Self: 'static + Send,
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{
	fn run(&mut self) -> Result<(), CacheError> {
		loop {
			let events = self.listener
				.try_iter()
				.collect::<Vec<_>>();

			for event in events {
				self.get_writer()?
					.write_chunk(&event)
					.map_err(|_| CacheError::Internal)?;
			}

			self.get_writer()?
				.flush().map_err(|_| CacheError::Internal)?;

			thread::sleep(POLL_DELAY);
		}
	}
}

impl<K, V, S> TraceWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{
	pub fn new(
		listener: Receiver<StackEvent<K>>,
		traces: Arc<RwLock<VecDeque<(Instant, File)>>>,
	) -> Self {
		TraceWorker {
			listener,

			traces,
			current_writer: None,

			_v_marker: PhantomData,
			_s_marker: PhantomData,
		}
	}

	fn get_writer(&mut self) -> Result<&mut BinaryWriter<StackEvent<K>>, CacheError> {
		self.refresh_traces()?;
		self.current_writer.as_mut().ok_or(CacheError::Internal)
	}

	/// Ensure all traces are younger than TRACE_MAX_AGE and the current
	/// writer points to the latest trace which is also younger than
	/// TRACE_REFRESH_AGE
	fn refresh_traces(&mut self) -> Result<(), CacheError> {
		while self.traces
			.read()
			.back()
			.is_some_and(|(created, _)| created.elapsed() > TRACE_MAX_AGE) {

			// remove any trace path that is older than TRACE_MAX_AGE
			self.traces.write().pop_front();
		}

		if self.traces
			.read()
			.back()
			.is_some_and(|(created, _)| created.elapsed() <= TRACE_REFRESH_AGE) {

			// the latest trace is still valid
			return Ok(());
		}

		// the latest trace is no longer valid, so create a new one
		let file = tempfile().map_err(|_| CacheError::Internal)?;

		// we need to keep a secondary instance of the file open to prevent it
		// from being deleted
		let file_clone = file.try_clone().map_err(|_| CacheError::Internal)?;

		let writer = BinaryWriter::<StackEvent<K>>::from_file(file)
			.map_err(|_| CacheError::Internal)?;

		self.traces
			.write()
			.push_back((Instant::now(), file_clone));

		self.current_writer = Some(writer);

		Ok(())
	}
}

unsafe impl<K, V, S> Send for TraceWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{}
