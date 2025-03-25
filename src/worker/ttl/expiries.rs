use std::{
	time::Instant,
	collections::BTreeMap,
};

use crate::{
	cache::HashedKey,
	object::{ExpireTime, get_expiry_from_ttl},
};

#[derive(Default)]
pub struct Expiries {
	map: BTreeMap<Instant, HashedKey>,
}

impl Expiries {
	pub fn has_within(&self, ttl: u32) -> bool {
		let Some((nearest_expiry, _)) = self.map.first_key_value() else {
			return false;
		};

		*nearest_expiry <= get_expiry_from_ttl(ttl)
	}

	pub fn insert(&mut self, key: HashedKey, expiry: ExpireTime) {
		let Some(expiry) = expiry else {
			return;
		};

		self.map.insert(expiry, key);
	}

	pub fn remove(&mut self, key: HashedKey, expiry: ExpireTime) {
		let Some(expiry) = expiry else {
			return;
		};

		if self.map.get(&expiry).is_none_or(|got_key| *got_key != key) {
			return;
		}

		self.map.remove(&expiry);
	}

	pub fn pop_expired(&mut self, now: Instant) -> Option<HashedKey> {
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
