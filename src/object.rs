mod eviction_map;

use kwik::utils;

use crate::{
	Policy,
	object::eviction_map::{
		NUM_POLICIES,
		EvictionMap,
		EvictionMapPolicy,
		LfuEvictionMap,
	},
};

pub struct Object<T: MemSize> {
	data: T,
	expiry: Option<u64>,

	eviction_maps: Vec<Option<EvictionMapPolicy>>,
}

pub trait MemSize {
	fn mem_size(&self) -> usize;
}

impl<T: MemSize> Object<T> {
	pub fn new(data: T, ttl: Option<u32>, policies: &[Policy]) -> Self {
		let expiry = match ttl {
			Some(0) | None => None,

			Some(ttl) => {
				let now = utils::timestamp();
				Some(ttl as u64 * 1000 + now)
			},
		};

		Object {
			data,
			expiry,

			eviction_maps: get_eviction_maps(policies),
		}
	}

	pub fn get_data(&self) -> &T {
		&self.data
	}

	pub fn get_size(&self) -> u64 {
		self.data.mem_size() as u64
	}

	pub fn get_expiry(&self) -> Option<u64> {
		self.expiry
	}

	pub fn insert_eviction_records(&mut self, cache_size: u64) {
		self.eviction_maps
			.iter_mut()
			.for_each(|eviction_map| {
				if let Some(eviction_map) = eviction_map {
					eviction_map.insert(cache_size);
				}
			});
	}

	pub fn update_eviction_records(&mut self) {
		self.eviction_maps
			.iter_mut()
			.for_each(|eviction_map| {
				if let Some(eviction_map) = eviction_map {
					eviction_map.update();
				}
			});
	}
}

fn get_eviction_maps(policies: &[Policy]) -> Vec<Option<EvictionMapPolicy>> {
	let mut eviction_maps = Vec::<Option<EvictionMapPolicy>>::with_capacity(NUM_POLICIES);

	for policy in policies {
		let index = policy.index();

		while eviction_maps.len() < index + 1 {
			eviction_maps.push(None);
		}

		match policy {
			Policy::Lfu => {
				eviction_maps[index] = Some(EvictionMapPolicy::Lfu(LfuEvictionMap::default()));
			},

			_ => todo!(),
		}
	}

	eviction_maps
}
