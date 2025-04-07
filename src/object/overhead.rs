use std::{
	mem,
	time::Instant,
};

use typesize::TypeSize;

use crate::{
	policy::PaperPolicy,
	object::{Object, ObjectSize},
};

pub struct OverheadManager {
	policies_overhead_per_object: ObjectSize,
	ttl_overhead_per_object: ObjectSize,
}

impl OverheadManager {
	pub fn new(policies: &[PaperPolicy]) -> Self {
		// the overheads are just rough estimates of the number of bytes per object

		let policies_overhead_per_object = policies
			.iter()
			.map(|policy| match policy {
				// 16 bytes for the HashMap entry 32 bytes for the HashList entry,
				// 8 bytes for the HashedKey, 4 bytes for the count
				PaperPolicy::Lfu => 60,

				// 32 bytes for the HashList entry, 8 bytes for the HashedKey
				PaperPolicy::Fifo => 40,

				// 32 bytes for the HashList entry, 8 bytes for the HashedKey
				PaperPolicy::Lru => 40,

				// 32 bytes for the HashList entry, 8 bytes for the HashedKey
				PaperPolicy::Mru => 40,

				// 32 bytes for the HashList entry, 8 bytes for the HashedKey,
				// 4 bytes for the object size
				PaperPolicy::TwoQ(_, _) => 44,
			})
			.sum();

		// the size of an Instant plus 8 bytes for the key in the BTreeMap
		let ttl_overhead_per_object = mem::size_of::<Instant>() as u32 + 8;

		OverheadManager {
			policies_overhead_per_object,
			ttl_overhead_per_object,
		}
	}

	pub fn total_size<K, V>(&self, object: &Object<K, V>) -> ObjectSize
	where
		K: TypeSize,
		V: TypeSize,
	{
		let mut total_size = object.total_size() + self.policies_overhead_per_object;

		if object.expiry().is_some() {
			total_size += self.ttl_overhead_per_object;
		}

		total_size
	}
}
