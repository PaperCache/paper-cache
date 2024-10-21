use std::{
	io,
	hash::Hash,
	time::{Instant, Duration},
};

use typesize::TypeSize;
use parking_lot::{Mutex, MutexGuard};
use tempfile::tempfile;

use kwik::file::{
	FileReader,
	FileWriter,
	binary::{BinaryReader, BinaryWriter, ReadChunk, WriteChunk},
};

use crate::worker::policy::event::StackEvent;

type Modifiers<K> = (BinaryReader<StackEvent<K>>, BinaryWriter<StackEvent<K>>);

// REFRESH_AGE must be less than MAX_AGE
const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const REFRESH_AGE: Duration = Duration::from_secs(60 * 60);

pub struct TraceFragment<K>
where
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
{
	created: Instant,
	modifiers: Mutex<Modifiers<K>>,
}

impl<K> TraceFragment<K>
where
	K: 'static + Copy + Eq + Hash + Send + Sync + TypeSize + ReadChunk + WriteChunk,
{
	pub fn new() -> io::Result<Self> {
		let reader_file = tempfile()?;
		let writer_file = reader_file.try_clone()?;

		let reader = BinaryReader::<StackEvent<K>>::from_file(reader_file)?;
		let writer = BinaryWriter::<StackEvent<K>>::from_file(writer_file)?;

		let fragment = TraceFragment {
			created: Instant::now(),
			modifiers: Mutex::new((reader, writer)),
		};

		Ok(fragment)
	}

	pub fn is_expired(&self) -> bool {
		self.created.elapsed() > MAX_AGE
	}

	pub fn is_valid(&self) -> bool {
		self.created.elapsed() <= REFRESH_AGE
	}

	pub fn lock(&self) -> MutexGuard<Modifiers<K>> {
		self.modifiers.lock()
	}
}
