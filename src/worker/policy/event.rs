/*
 * Copyright (c) Kia Shakiba
 *
 * This source code is licensed under the GNU AGPLv3 license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::{cmp, io};

use kwik::file::binary::{ReadChunk, SizedChunk, WriteChunk};

use crate::{CacheSize, HashedKey, object::ObjectSize, worker::WorkerEvent};

#[derive(Clone)]
pub enum StackEvent {
	Get(HashedKey),
	Set(HashedKey, ObjectSize),
	Del(HashedKey),
	Wipe,
	Resize(CacheSize),
}

pub enum TraceEvent {
	Get(HashedKey),
	Set(HashedKey, ObjectSize),
	Del(HashedKey),
	Resize(CacheSize),
}

impl StackEvent {
	pub fn maybe_from_worker_event(worker_event: &WorkerEvent) -> Option<Self> {
		let event = match worker_event {
			WorkerEvent::Get(key, hit) if *hit => StackEvent::Get(*key),
			WorkerEvent::Set(key, size, _, _) => StackEvent::Set(*key, *size),
			WorkerEvent::Del(key, _) => StackEvent::Del(*key),
			WorkerEvent::Wipe => StackEvent::Wipe,
			WorkerEvent::Resize(size) => StackEvent::Resize(*size),

			_ => return None,
		};

		Some(event)
	}
}

impl TraceEvent {
	pub fn maybe_from_stack_event(stack_event: &StackEvent) -> Option<Self> {
		let event = match stack_event {
			StackEvent::Get(key) => TraceEvent::Get(*key),
			StackEvent::Set(key, size) => TraceEvent::Set(*key, *size),
			StackEvent::Del(key) => TraceEvent::Del(*key),
			StackEvent::Resize(size) => TraceEvent::Resize(*size),

			_ => return None,
		};

		Some(event)
	}
}

impl SizedChunk for TraceEvent {
	fn chunk_size() -> usize {
		let set_size = HashedKey::chunk_size() + ObjectSize::chunk_size() + 1;
		let resize_size = CacheSize::chunk_size() + 1;

		cmp::max(set_size, resize_size)
	}
}

impl ReadChunk for TraceEvent {
	fn from_chunk(buf: &[u8]) -> std::io::Result<Self> {
		let event = match buf[0] {
			EventByte::GET => {
				let key = HashedKey::from_chunk(&buf[1..HashedKey::chunk_size() + 1])?;
				TraceEvent::Get(key)
			},

			EventByte::SET => {
				let key = HashedKey::from_chunk(&buf[1..HashedKey::chunk_size() + 1])?;
				let size = ObjectSize::from_chunk(&buf[HashedKey::chunk_size() + 1..])?;
				TraceEvent::Set(key, size)
			},

			EventByte::DEL => {
				let key = HashedKey::from_chunk(&buf[1..HashedKey::chunk_size() + 1])?;
				TraceEvent::Del(key)
			},

			EventByte::RESIZE => {
				let size = CacheSize::from_chunk(&buf[1..CacheSize::chunk_size() + 1])?;
				TraceEvent::Resize(size)
			},

			_ => unreachable!(),
		};

		Ok(event)
	}
}

impl WriteChunk for TraceEvent {
	fn as_chunk(&self, buf: &mut Vec<u8>) -> io::Result<()> {
		match self {
			TraceEvent::Get(key) => {
				buf.push(EventByte::GET);
				key.as_chunk(buf)?;

				let remaining = TraceEvent::chunk_size() - HashedKey::chunk_size() - 1;
				let zeros = std::iter::repeat_n(0, remaining);
				buf.extend(zeros);
			},

			TraceEvent::Set(key, size) => {
				buf.push(EventByte::SET);
				key.as_chunk(buf)?;
				size.as_chunk(buf)?;
			},

			TraceEvent::Del(key) => {
				buf.push(EventByte::DEL);
				key.as_chunk(buf)?;

				let remaining = TraceEvent::chunk_size() - HashedKey::chunk_size() - 1;
				let zeros = std::iter::repeat_n(0, remaining);
				buf.extend(zeros);
			},

			TraceEvent::Resize(size) => {
				buf.push(EventByte::RESIZE);
				size.as_chunk(buf)?;

				let remaining = TraceEvent::chunk_size() - CacheSize::chunk_size() - 1;
				let zeros = std::iter::repeat_n(0, remaining);
				buf.extend(zeros);
			},
		}

		Ok(())
	}
}

struct EventByte;

impl EventByte {
	const GET: u8 = 0;
	const SET: u8 = 1;
	const DEL: u8 = 2;
	const RESIZE: u8 = 3;
}
