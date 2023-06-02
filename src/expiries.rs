use std::hash::Hash;
use std::collections::BTreeMap;
use rustc_hash::FxHashSet;

pub struct Expiries<T: Eq + Hash> {
	map: BTreeMap<u64, FxHashSet<T>>,
}

impl<T: Eq + Hash> Expiries<T> {
	pub fn new() -> Self {
		Expiries {
			map: BTreeMap::new(),
		}
	}

	pub fn insert(&mut self, key: T, expiry: Option<u64>) {
		let expiry = match expiry {
			Some(expiry) => expiry,

			None => {
				return;
			},
		};

		match self.map.get_mut(&expiry) {
			Some(keys) => {
				keys.insert(key);
			},

			None => {
				let mut keys = FxHashSet::default();
				keys.insert(key);

				self.map.insert(expiry, keys);
			},
		}
	}

	pub fn remove(&mut self, key: &T, expiry: &Option<u64>) {
		let expiry = match expiry {
			Some(expiry) => expiry,

			None => {
				return;
			},
		};

		match self.map.get_mut(&expiry) {
			Some(keys) => {
				keys.remove(key);
			},

			None => {
				self.map.remove(expiry);
			},
		}
	}

	pub fn next_timestamp(&self) -> Option<u64> {
	}

	pub fn clear(&mut self) {
		self.map.clear();
	}
}
