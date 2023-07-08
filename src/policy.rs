#[derive(PartialEq, Clone, Copy)]
pub enum Policy {
	Lru,
	Mru,
	Lfu,
}

impl Policy {
	pub fn index(&self) -> usize {
		match self {
			Policy::Lru => 0,
			Policy::Mru => 1,
			Policy::Lfu => 2,
		}
	}
}
