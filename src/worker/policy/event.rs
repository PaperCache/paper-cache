use std::{io, cmp};
use kwik::file::binary::{SizedChunk, ReadChunk, WriteChunk};

use crate::{
	CacheSize,
	HashedKey,
	object::ObjectSize,
	worker::WorkerEvent,
};

#[derive(Clone)]
pub enum StackEvent {
	Get(HashedKey),
	Set(HashedKey, ObjectSize),
	Del(HashedKey),
	Wipe,
	Resize(CacheSize),
}

struct EventByte;

impl StackEvent {
	pub fn maybe_from_worker_event(worker_event: &WorkerEvent) -> Option<Self> {
		let event = match worker_event {
			WorkerEvent::Get(key, hit) if *hit => StackEvent::Get(*key),
			WorkerEvent::Set(key, size, _, _) => StackEvent::Set(*key, *size),
			WorkerEvent::Del(key, _, _) => StackEvent::Del(*key),
			WorkerEvent::Wipe => StackEvent::Wipe,
			WorkerEvent::Resize(size) => StackEvent::Resize(*size),

			_ => return None,
		};

		Some(event)
	}
}

impl SizedChunk for StackEvent {
	fn chunk_size() -> usize {
		let set_size = HashedKey::chunk_size() + ObjectSize::chunk_size() + 1;
		let resize_size = CacheSize::chunk_size() + 1;

		cmp::max(set_size, resize_size)
	}
}

impl ReadChunk for StackEvent {
	fn from_chunk(buf: &[u8]) -> std::io::Result<Self> {
		let event = match buf[0] {
			EventByte::GET => {
				let key = HashedKey::from_chunk(&buf[1..HashedKey::chunk_size() + 1])?;
				StackEvent::Get(key)
			},

			EventByte::SET => {
				let key = HashedKey::from_chunk(&buf[1..HashedKey::chunk_size() + 1])?;
				let size = ObjectSize::from_chunk(&buf[HashedKey::chunk_size() + 1..])?;
				StackEvent::Set(key, size)
			},

			EventByte::DEL => {
				let key = HashedKey::from_chunk(&buf[1..HashedKey::chunk_size() + 1])?;
				StackEvent::Del(key)
			},

			EventByte::WIPE => StackEvent::Wipe,

			EventByte::RESIZE => {
				let size = CacheSize::from_chunk(&buf[1..CacheSize::chunk_size() + 1])?;
				StackEvent::Resize(size)
			},

			_ => unreachable!(),
		};

		Ok(event)
	}
}

impl WriteChunk for StackEvent {
	fn as_chunk(&self, buf: &mut Vec<u8>) -> io::Result<()> {
		match self {
			StackEvent::Get(key) => {
				buf.push(EventByte::GET);
				key.as_chunk(buf)?;

				let remaining = StackEvent::chunk_size() - HashedKey::chunk_size() - 1;
				let zeros = std::iter::repeat_n(0, remaining);
				buf.extend(zeros);
			},

			StackEvent::Set(key, size) => {
				buf.push(EventByte::SET);
				key.as_chunk(buf)?;
				size.as_chunk(buf)?;
			},

			StackEvent::Del(key) => {
				buf.push(EventByte::DEL);
				key.as_chunk(buf)?;

				let remaining = StackEvent::chunk_size() - HashedKey::chunk_size() - 1;
				let zeros = std::iter::repeat_n(0, remaining);
				buf.extend(zeros);
			},

			StackEvent::Wipe => {
				buf.push(EventByte::WIPE);

				let remaining = StackEvent::chunk_size() - 1;
				let zeros = std::iter::repeat_n(0, remaining);
				buf.extend(zeros);
			},

			StackEvent::Resize(size) => {
				buf.push(EventByte::RESIZE);
				size.as_chunk(buf)?;

				let remaining = StackEvent::chunk_size() - CacheSize::chunk_size() - 1;
				let zeros = std::iter::repeat_n(0, remaining);
				buf.extend(zeros);
			},
		}

		Ok(())
	}
}

impl EventByte {
	const GET: u8		= 0;
	const SET: u8		= 1;
	const DEL: u8		= 2;
	const WIPE: u8		= 3;
	const RESIZE: u8	= 4;
}
