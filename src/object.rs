use kwik::utils;

pub struct Object<T: MemSize> {
	data: T,
	expiry: Option<u64>,
}

pub trait MemSize {
	fn mem_size(&self) -> usize;
}

impl<T: MemSize> Object<T> {
	pub fn new(data: T, ttl: Option<u32>) -> Self {
		let expiry = match ttl {
			Some(0) | None => None,

			Some(ttl) => {
				let now = utils::timestamp();
				Some(ttl as u64 * 1000 + now)
			},
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

	pub fn get_expiry(&self) -> Option<u64> {
		self.expiry
	}
}
