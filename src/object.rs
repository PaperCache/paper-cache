use std::mem;

pub struct Object<T> {
	data: T,
}

impl<T> Object<T> {
	pub fn new(data: T) -> Self {
		Object {
			data
		}
	}

	pub fn get_data(&self) -> &T {
		&self.data
	}

	pub fn get_size(&self) -> usize {
		mem::size_of_val(&self.data)
	}
}
