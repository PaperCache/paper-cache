use std::mem;
use kwik::utils;

pub struct Object<T> {
	data: T,
	expiry: u64,
}

impl<T> Object<T> {
	pub fn new(data: T, ttl: Option<u32>) -> Self {
		let now = utils::timestamp();

		let expiry = match ttl {
			Some(ttl) => ttl as u64 * 1000 + now,
			None => 0,
		};

		Object {
			data,
			expiry,
		}
	}

	pub fn get_data(&self) -> &T {
		&self.data
	}

	pub fn get_size(&self) -> usize {
		mem::size_of_val(&self.data)
	}

	pub fn is_expired(&self) -> bool {
		let now = utils::timestamp();

		if self.expiry == 0 {
			return false;
		}

		self.expiry < now
	}
}
