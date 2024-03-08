use std::sync::Arc;
use kwik::utils;

pub struct Object<T>
where
	T: MemSize,
{
	data: Arc<T>,
	expiry: Option<u64>,
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

			Some(ttl) => {
				let now = utils::timestamp();
				Some(u64::from(ttl) * 1000 + now)
			},
		};

		Object {
			data: Arc::new(data),
			expiry,
		}
	}

	pub fn data(&self) -> Arc<T> {
		self.data.clone()
	}

	pub fn size(&self) -> u64 {
		self.data.mem_size() as u64
	}

	pub fn expiry(&self) -> Option<u64> {
		self.expiry
	}
}
