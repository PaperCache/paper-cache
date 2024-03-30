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
	fn mem_size(&self) -> ObjectSize;
}

impl<T> Object<T>
where
	T: MemSize,
{
	pub fn new(data: T, ttl: Option<u32>) -> Self {
		let expiry = match ttl {
			Some(0) | None => None,
			Some(ttl) => Some(get_expiry_from_ttl(ttl)),
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
		self.data.mem_size()
	}

	pub fn expiry(&self) -> ExpireTime {
		self.expiry
	}

	pub fn is_expired(&self) -> bool {
		self.expiry.is_some_and(|expiry| expiry <= utils::timestamp())
	}

	pub fn expires(&mut self, ttl: Option<u32>) {
		self.expiry = match ttl {
			Some(0) | None => None,
			Some(ttl) => Some(utils::timestamp() + u64::from(ttl) * 1000),
		};
	}
}

pub fn get_expiry_from_ttl(ttl: u32) -> u64 {
	utils::timestamp() + u64::from(ttl) * 1000
}
