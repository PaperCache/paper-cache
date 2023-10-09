use crate::object::eviction_map::EvictionMap;

pub struct LfuEvictionMap {
	global_count: u64,
	map: Vec<EvictionRecord>,
}

struct EvictionRecord {
	size: u64,
	count: u64,
}

impl EvictionMap for LfuEvictionMap {
	fn insert(&mut self, size: u64) {
		while self.map.last().is_some_and(|record| record.size <= size) {
			self.map.pop();
		}

		self.map.push(EvictionRecord::new(size, self.global_count));
	}

	fn update(&mut self) {
		self.global_count += 1;
	}
}

impl Default for LfuEvictionMap {
	fn default() -> Self {
		LfuEvictionMap {
			global_count: 1,
			map: Vec::new(),
		}
	}
}

impl EvictionRecord {
	fn new(size: u64, count: u64) -> Self {
		EvictionRecord {
			size,
			count,
		}
	}
}
