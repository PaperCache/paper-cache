use std::{
	hash::{Hash, BuildHasher},
	collections::{BTreeMap, HashSet},
};

use crate::object::{ExpireTime, get_expiry_from_ttl};

pub struct Expiries<K, S>
where
	K: Copy + Eq + Hash,
	S: Clone + BuildHasher,
{
	map: BTreeMap<u64, HashSet<K, S>>,
	hasher: S,
}

impl<K, S> Expiries<K, S>
where
	K: Copy + Eq + Hash,
	S: Clone + BuildHasher,
{
	pub fn with_hasher(hasher: S) -> Self {
		Expiries {
			map: BTreeMap::default(),
			hasher,
		}
	}

	pub fn has_within(&self, ttl: u32) -> bool {
		let Some((nearest_expiry, _)) = self.map.first_key_value() else {
			return false;
		};

		*nearest_expiry <= get_expiry_from_ttl(ttl)
	}

	pub fn insert(&mut self, key: K, expiry: ExpireTime) {
		let Some(expiry) = expiry else {
			return;
		};

		if let Some(keys) = self.map.get_mut(&expiry) {
			keys.insert(key);
		} else {
			let mut keys = HashSet::with_hasher(self.hasher.clone());
			keys.insert(key);

			self.map.insert(expiry, keys);
		}
	}

	pub fn remove(&mut self, key: K, expiry: ExpireTime) {
		let Some(expiry) = expiry else {
			return;
		};

		match self.map.get_mut(&expiry) {
			Some(keys) => {
				keys.remove(&key);
			},

			None => {
				self.map.remove(&expiry);
			},
		}
	}

	pub fn expired(&mut self, now: u64) -> Option<HashSet<K, S>> {
		let first_expiry = self.map
			.first_key_value()
			.map(|(expiry, _)| expiry)?;

		if *first_expiry > now {
			return None;
		}

		self.map.pop_first().map(|(_, keys)| keys)
	}

	pub fn clear(&mut self) {
		self.map.clear();
	}
}
