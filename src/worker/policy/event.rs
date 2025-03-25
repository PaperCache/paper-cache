use std::io;
use kwik::file::binary::{SizedChunk, ReadChunk, WriteChunk};

use crate::{
	cache::HashedKey,
	worker::WorkerEvent,
};

#[derive(Clone)]
pub enum StackEvent {
	Get(HashedKey),
	Set(HashedKey),
	Del(HashedKey),
	Wipe,
}

struct EventByte;

impl StackEvent {
	pub fn maybe_from_worker_event(worker_event: &WorkerEvent) -> Option<Self> {
		let event = match worker_event {
			WorkerEvent::Get(key, hit) if *hit => StackEvent::Get(*key),
			WorkerEvent::Set(key, _, _, _) => StackEvent::Set(*key),
			WorkerEvent::Del(key, _, _) => StackEvent::Del(*key),
			WorkerEvent::Wipe => StackEvent::Wipe,

			_ => return None,
		};

		Some(event)
	}
}

impl SizedChunk for StackEvent {
	fn size() -> usize {
		HashedKey::size() + 1
	}
}

impl ReadChunk for StackEvent {
	fn from_chunk(buf: &[u8]) -> std::io::Result<Self> {
		let event = match buf[0] {
			EventByte::GET => {
				let key = HashedKey::from_chunk(&buf[1..])?;
				StackEvent::Get(key)
			},

			EventByte::SET => {
				let key = HashedKey::from_chunk(&buf[1..])?;
				StackEvent::Set(key)
			},

			EventByte::DEL => {
				let key = HashedKey::from_chunk(&buf[1..])?;
				StackEvent::Del(key)
			},

			EventByte::WIPE => StackEvent::Wipe,

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
			},

			StackEvent::Set(key) => {
				buf.push(EventByte::SET);
				key.as_chunk(buf)?;
			},

			StackEvent::Del(key) => {
				buf.push(EventByte::DEL);
				key.as_chunk(buf)?;
			},

			StackEvent::Wipe => {
				buf.push(EventByte::WIPE);
				buf.extend_from_slice(&vec![0u8; HashedKey::size()]);
			},
		}

		Ok(())
	}
}

impl EventByte {
	const GET: u8	= 0;
	const SET: u8	= 1;
	const DEL: u8	= 2;
	const WIPE: u8	= 3;
}
