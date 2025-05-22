/*
 * Copyright (c) Kia Shakiba
 *
 * This source code is licensed under the GNU AGPLv3 license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::{
	io,
	time::{Instant, Duration},
};

use parking_lot::{Mutex, MutexGuard};
use tempfile::tempfile;

use kwik::file::{
	FileReader,
	FileWriter,
	binary::{BinaryReader, BinaryWriter},
};

use crate::worker::policy::event::StackEvent;

type Modifiers = (BinaryReader<StackEvent>, BinaryWriter<StackEvent>);

// REFRESH_AGE must be less than MAX_AGE
const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const REFRESH_AGE: Duration = Duration::from_secs(60 * 60);

pub struct TraceFragment {
	created: Instant,
	modifiers: Mutex<Modifiers>,
}

impl TraceFragment {
	pub fn new() -> io::Result<Self> {
		let reader_file = tempfile()?;
		let writer_file = reader_file.try_clone()?;

		let reader = BinaryReader::<StackEvent>::from_file(reader_file)?;
		let writer = BinaryWriter::<StackEvent>::from_file(writer_file)?;

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

	pub fn lock(&self) -> MutexGuard<Modifiers> {
		self.modifiers.lock()
	}
}
