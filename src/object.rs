use kwik::utils;

pub struct Object<T: MemSize> {
	data: T,
	expiry: u64,
}

pub trait MemSize {
	fn mem_size(&self) -> usize;
}

impl<T: MemSize> Object<T> {
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

	pub fn get_size(&self) -> u64 {
		self.data.mem_size() as u64
	}

	pub fn get_expiry(&self) -> &u64 {
		&self.expiry
	}
}
