use std::{
	rc::Rc,
	hash::Hash,
	collections::BTreeMap,
};

use rustc_hash::FxHashSet;

pub struct Expiries<K>
where
	K: Eq + Hash,
{
	map: BTreeMap<u64, FxHashSet<Rc<K>>>,
}

impl<K> Expiries<K>
where
	K: Eq + Hash,
{
	pub fn new() -> Self {
		Expiries {
			map: BTreeMap::new(),
		}
	}

	pub fn insert(&mut self, key: &Rc<K>, expiry: Option<u64>) {
		let expiry = match expiry {
			Some(expiry) => expiry,
			None => return,
		};

		match self.map.get_mut(&expiry) {
			Some(keys) => {
				keys.insert(Rc::clone(key));
			},

			None => {
				let mut keys = FxHashSet::default();
				keys.insert(Rc::clone(key));

				self.map.insert(expiry, keys);
			},
		}
	}

	pub fn remove(&mut self, key: &K, expiry: Option<u64>) {
		let expiry = match expiry {
			Some(expiry) => expiry,
			None => return,
		};

		match self.map.get_mut(&expiry) {
			Some(keys) => {
				keys.remove(key);
			},

			None => {
				self.map.remove(&expiry);
			},
		}
	}

	pub fn expired(&mut self, now: u64) -> Option<FxHashSet<Rc<K>>> {
		let first_expiry = match self.map.first_key_value() {
			Some((expiry, _)) => expiry,

			None => {
				return None;
			},
		};

		if *first_expiry > now {
			return None;
		}

		self.map.pop_first().map(|(_, keys)| keys)
	}

	pub fn clear(&mut self) {
		self.map.clear();
	}
}
