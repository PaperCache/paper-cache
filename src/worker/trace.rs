use std::{
	mem,
	thread,
	sync::{Arc, RwLock},
	io::{self, Cursor},
	hash::{Hash, BuildHasher},
	time::{Instant, Duration},
	fs::File,
	collections::VecDeque,
	marker::PhantomData,
};

use typesize::TypeSize;
use tempfile::tempfile;
use byteorder::{ReadBytesExt, LittleEndian};

use kwik::file::{
	FileWriter,
	binary::{BinaryWriter, SizedChunk, ReadChunk, WriteChunk},
};

use crate::{
	error::CacheError,
	worker::{Worker, WorkerEvent, WorkerReceiver},
	object::ObjectSize,
};

const TRACE_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const TRACE_REFRESH_AGE: Duration = Duration::from_secs(60 * 60);
const POLL_DELAY: Duration = Duration::from_secs(1);

pub struct TraceWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{
	listener: WorkerReceiver<K>,

	traces: Arc<RwLock<VecDeque<(Instant, File)>>>,
	current_writer: Option<BinaryWriter<Access<K>>>,

	_v_marker: PhantomData<V>,
	_s_marker: PhantomData<S>,
}

pub struct Access<K> {
	key: K,
	size: ObjectSize,
}

impl<K, V, S> Worker<K, V, S> for TraceWorker<K, V, S>
where
	Self: 'static + Send,
	K: 'static + Copy + Eq + Hash + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{
	fn run(&mut self) -> Result<(), CacheError> {
		loop {
			let events = self.listener
				.try_iter()
				.collect::<Vec<WorkerEvent<K>>>();

			for event in events {
				match event {
					WorkerEvent::Get(key, hit) if hit => self.handle_get(key)?,
					WorkerEvent::Set(key, size, _, _) => self.handle_set(key, size)?,
					WorkerEvent::Wipe => self.handle_wipe()?,
					WorkerEvent::Policy(_) => self.handle_policy()?,

					_ => {},
				}
			}

			thread::sleep(POLL_DELAY);
		}
	}
}

impl<K, V, S> TraceWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{
	pub fn new(
		listener: WorkerReceiver<K>,
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

	fn handle_get(&mut self, key: K) -> Result<(), CacheError> {
		let access = Access {
			key,
			size: 0,
		};

		self.get_writer()?
			.write_chunk(&access)
			.map_err(|_| CacheError::Internal)
	}

	fn handle_set(&mut self, key: K, size: ObjectSize) -> Result<(), CacheError> {
		let access = Access {
			key,
			size,
		};

		self.get_writer()?
			.write_chunk(&access)
			.map_err(|_| CacheError::Internal)
	}

	fn handle_wipe(&mut self) -> Result<(), CacheError> {
		self.traces
			.write().map_err(|_| CacheError::Internal)?
			.clear();

		self.current_writer = None;

		Ok(())
	}

	fn handle_policy(&mut self) -> Result<(), CacheError> {
		self.get_writer()?
			.flush().map_err(|_| CacheError::Internal)
	}

	fn get_writer(&mut self) -> Result<&mut BinaryWriter<Access<K>>, CacheError> {
		self.refresh_traces()?;
		self.current_writer.as_mut().ok_or(CacheError::Internal)
	}

	/// Ensure all traces are younger than TRACE_MAX_AGE and the current
	/// writer points to the latest trace which is also younger than
	/// TRACE_REFRESH_AGE
	fn refresh_traces(&mut self) -> Result<(), CacheError> {
		while self.traces
			.read().map_err(|_| CacheError::Internal)?
			.back().is_some_and(|(created, _)| created.elapsed() > TRACE_MAX_AGE) {

			// remove any trace path that is older than TRACE_MAX_AGE
			self.traces
				.write().map_err(|_| CacheError::Internal)?
				.pop_front();
		}

		if self.traces
			.read().map_err(|_| CacheError::Internal)?
			.back().is_some_and(|(created, _)| created.elapsed() <= TRACE_REFRESH_AGE) {

			// the latest trace is still valid
			return Ok(());
		}

		// the latest trace is no longer valid, so create a new one
		let file = tempfile().map_err(|_| CacheError::Internal)?;

		// we need to keep a secondary instance of the file open to prevent it
		// from being deleted
		let file_clone = file.try_clone().map_err(|_| CacheError::Internal)?;

		let writer = BinaryWriter::<Access<K>>::from_file(file)
			.map_err(|_| CacheError::Internal)?;

		self.traces
			.write().map_err(|_| CacheError::Internal)?
			.push_back((Instant::now(), file_clone));

		self.current_writer = Some(writer);

		Ok(())
	}
}

impl<K> Access<K>
where
	K: Copy,
{
	pub fn key(&self) -> K {
		self.key
	}

	pub fn size(&self) -> ObjectSize {
		self.size
	}
}

impl<K> SizedChunk for Access<K>
where
	K: WriteChunk,
{
	fn size() -> usize {
		K::size() + mem::size_of::<ObjectSize>()
	}
}

impl<K> ReadChunk for Access<K>
where
	K: ReadChunk + WriteChunk,
{
	fn new(buf: &[u8]) -> io::Result<Self> {
		let key = K::new(buf)?;
		let mut rdr = Cursor::new(&buf[K::size()..]);
		let size = rdr.read_u64::<LittleEndian>()?;

		let access = Access {
			key,
			size,
		};

		Ok(access)
	}
}

impl<K> WriteChunk for Access<K>
where
	K: WriteChunk,
{
	fn as_chunk(&self, buf: &mut Vec<u8>) -> io::Result<()> {
		self.key.as_chunk(buf)?;
		buf.extend_from_slice(&self.size.to_le_bytes());

		Ok(())
	}
}

unsafe impl<K, V, S> Send for TraceWorker<K, V, S>
where
	K: 'static + Copy + Eq + Hash + Sync + TypeSize + ReadChunk + WriteChunk,
	V: 'static + Sync + TypeSize,
	S: Default + Clone + BuildHasher,
{}
