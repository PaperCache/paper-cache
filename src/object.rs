use std::sync::Arc;
use kwik::utils;

pub type ObjectSize = u64;
pub type ExpireTime = Option<u64>;

pub struct Object<T>
where
	T: MemSize,
{
	data: Arc<T>,
	expiry: ExpireTime,
}

pub trait MemSize {
	fn mem_size(&self) -> usize;
}

impl<T> Object<T>
where
	T: MemSize,
{
	pub fn new(data: T, ttl: Option<u32>) -> Self {
		let expiry = match ttl {
			Some(0) | None => None,
			Some(ttl) => Some(utils::timestamp() + u64::from(ttl) * 1000),
		};

		Object {
			data: Arc::new(data),
			expiry,
		}
	}

	pub fn data(&self) -> Arc<T> {
		self.data.clone()
	}

	pub fn size(&self) -> ObjectSize {
		self.data.mem_size() as ObjectSize
	}

	pub fn expiry(&self) -> ExpireTime {
		self.expiry
	}
}
