pub mod overhead;

use std::{
	mem,
	sync::Arc,
	time::{Instant, Duration},
};

use typesize::TypeSize;

pub type ObjectSize = u32;
pub type ExpireTime = Option<Instant>;

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
		(self.data.get_size() + mem::size_of::<ExpireTime>()) as ObjectSize
	}

	pub fn expiry(&self) -> ExpireTime {
		self.expiry
	}

	pub fn is_expired(&self) -> bool {
		self.expiry.is_some_and(|expiry| expiry <= Instant::now())
	}

	pub fn expires(&mut self, ttl: Option<u32>) {
		self.expiry = match ttl {
			Some(0) | None => None,
			Some(ttl) => Some(get_expiry_from_ttl(ttl)),
		};
	}
}

pub fn get_expiry_from_ttl(ttl: u32) -> Instant {
	Instant::now() + Duration::from_secs(ttl.into())
}
