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
