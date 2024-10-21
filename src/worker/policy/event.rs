use std::{
	io,
	hash::Hash,
};

use kwik::file::binary::{SizedChunk, ReadChunk, WriteChunk};
use crate::worker::WorkerEvent;

#[derive(Clone)]
pub enum StackEvent<K>
where
	K: Copy + Eq + Hash + ReadChunk + WriteChunk,
{
	Get(K),
	Set(K),
	Del(K),
	Wipe,
}

struct EventByte;

impl<K> StackEvent<K>
where
	K: Copy + Eq + Hash + ReadChunk + WriteChunk,
{
	pub fn maybe_from_worker_event(worker_event: &WorkerEvent<K>) -> Option<Self> {
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

impl<K> SizedChunk for StackEvent<K>
where
	K: Copy + Eq + Hash + ReadChunk + WriteChunk,
{
	fn size() -> usize {
		K::size() + 1
	}
}

impl<K> ReadChunk for StackEvent<K>
where
	K: Copy + Eq + Hash + ReadChunk + WriteChunk,
{
	fn from_chunk(buf: &[u8]) -> std::io::Result<Self> {
		let event = match buf[0] {
			EventByte::GET => {
				let key = K::from_chunk(&buf[1..])?;
				StackEvent::Get(key)
			},

			EventByte::SET => {
				let key = K::from_chunk(&buf[1..])?;
				StackEvent::Set(key)
			},

			EventByte::DEL => {
				let key = K::from_chunk(&buf[1..])?;
				StackEvent::Del(key)
			},

			EventByte::WIPE => StackEvent::Wipe,

			_ => unreachable!(),
		};

		Ok(event)
	}
}

impl<K> WriteChunk for StackEvent<K>
where
	K: Copy + Eq + Hash + ReadChunk + WriteChunk,
{
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
				buf.extend_from_slice(&vec![0u8; K::size()]);
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
