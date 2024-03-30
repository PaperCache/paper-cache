use std::{
	hash::Hash,
	collections::BTreeMap,
};

use rustc_hash::FxHashSet;
use crate::object::{ExpireTime, get_expiry_from_ttl};

pub struct Expiries<K>
where
	K: Copy + Eq + Hash,
{
	map: BTreeMap<u64, FxHashSet<K>>,
}

impl<K> Expiries<K>
where
	K: Copy + Eq + Hash,
{
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
			let mut keys = FxHashSet::default();
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

	pub fn expired(&mut self, now: u64) -> Option<FxHashSet<K>> {
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

impl<K> Default for Expiries<K>
where
	K: Copy + Eq + Hash,
{
	fn default() -> Self {
		Expiries {
			map: BTreeMap::default(),
		}
	}
}
