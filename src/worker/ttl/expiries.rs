use std::{
	time::Instant,
	collections::BTreeMap,
};

use crate::object::{ExpireTime, get_expiry_from_ttl};

pub struct Expiries<K>
where
	K: Copy + Eq,
{
	map: BTreeMap<Instant, K>,
}

impl<K> Expiries<K>
where
	K: Copy + Eq,
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

		self.map.insert(expiry, key);
	}

	pub fn remove(&mut self, key: K, expiry: ExpireTime) {
		let Some(expiry) = expiry else {
			return;
		};

		if self.map.get(&expiry).is_none_or(|got_key| *got_key != key) {
			return;
		}

		self.map.remove(&expiry);
	}

	pub fn pop_expired(&mut self, now: Instant) -> Option<K> {
		let first_expiry = self.map
			.first_key_value()
			.map(|(expiry, _)| expiry)?;

		if *first_expiry > now {
			return None;
		}

		self.map.pop_first().map(|(_, key)| key)
	}

	pub fn clear(&mut self) {
		self.map.clear();
	}
}

impl<K> Default for Expiries<K>
where
	K: Copy + Eq,
{
	fn default() -> Self {
		Expiries {
			map: BTreeMap::default(),
		}
	}
}
