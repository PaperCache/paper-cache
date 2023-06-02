use std::cmp::{Ord, Ordering, PartialEq};

pub struct Expiry<T> {
	key: T,
	timestamp: u64,
}

impl<T> Expiry<T> {
	pub fn new(key: T, timestamp: Option<u64>) -> Option<Self> {
		match timestamp {
			Some(timestamp) => {
				Some(Expiry {
					key,
					timestamp,
				})
			},

			None => None,
		}

	}
}

impl<T: PartialEq> Ord for Expiry<T> {
	fn cmp(&self, other: &Self) -> Ordering {
		self.partial_cmp(other).unwrap()
	}
}

impl<T: PartialEq> PartialOrd for Expiry<T> {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		other.timestamp.partial_cmp(&self.timestamp)
	}
}

impl<T: PartialEq> PartialEq for Expiry<T> {
	fn eq(&self, other: &Self) -> bool {
		self.key == other.key && self.timestamp == other.timestamp
	}
}

impl<T: PartialEq> Eq for Expiry<T> {}
