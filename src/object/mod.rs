pub mod overhead;

use std::sync::Arc;
use typesize::TypeSize;
use kwik::time;

pub type ObjectSize = u64;
pub type ExpireTime = Option<u64>;

pub struct Object<T>
where
	T: TypeSize,
{
	data: Arc<T>,
	expiry: ExpireTime,
}

impl<T> Object<T>
where
	T: TypeSize,
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

	fn total_size(&self) -> ObjectSize {
		(self.data.get_size() + self.expiry.get_size()) as ObjectSize
	}

	pub fn expiry(&self) -> ExpireTime {
		self.expiry
	}

	pub fn is_expired(&self) -> bool {
		self.expiry.is_some_and(|expiry| expiry <= time::timestamp())
	}

	pub fn expires(&mut self, ttl: Option<u32>) {
		self.expiry = match ttl {
			Some(0) | None => None,
			Some(ttl) => Some(time::timestamp() + u64::from(ttl) * 1000),
		};
	}
}

pub fn get_expiry_from_ttl(ttl: u32) -> u64 {
	time::timestamp() + u64::from(ttl) * 1000
}
