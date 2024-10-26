use std::{
	mem,
	time::Instant,
};

use typesize::TypeSize;

use crate::{
	cache::POLICIES,
	policy::PaperPolicy,
	object::{Object, ObjectSize},
};

pub struct OverheadManager {
	policies_overhead_per_object: ObjectSize,
	ttl_overhead_per_object: ObjectSize,
}

impl OverheadManager {
	pub fn total_size<K, V>(&self, key: K, object: &Object<V>) -> ObjectSize
	where
		K: TypeSize,
		V: TypeSize,
	{
		let mut total_size = key.get_size() as ObjectSize
			+ object.total_size()
			+ self.policies_overhead_per_object;

		if object.expiry().is_some() {
			total_size += self.ttl_overhead_per_object;
		}

		total_size
	}
}

impl Default for OverheadManager {
	fn default() -> Self {
		// the overheads are just rough estimates of the number of bytes per object

		let policies_overhead_per_object = POLICIES
			.iter()
			.map(|policy| match policy {
				// 8 bytes for the key of the HashMap, 16 bytes for KeyIndex, 8 bytes for the CountList
				PaperPolicy::Lfu => 32,

				// 16 bytes for the key and value of the HashMap, 8 bytes for the VecList
				PaperPolicy::Fifo => 24,

				// 16 bytes for the key and value of the HashMap, 8 bytes for the VecList
				PaperPolicy::Lru => 24,

				// 16 bytes for the key and value of the HashMap, 8 bytes for the VecList
				PaperPolicy::Mru => 24,
			})
			.sum();

		// the size of an Instant plus 8 bytes for the key in the BTreeMap
		let ttl_overhead_per_object = mem::size_of::<Instant>() as u32 + 8;

		OverheadManager {
			policies_overhead_per_object,
			ttl_overhead_per_object,
		}
	}
}
