#[derive(PartialEq, Clone, Copy)]
pub enum Policy {
	Lfu,
	Lru,
	Mru,
}

impl Policy {
	pub fn index(&self) -> usize {
		match self {
			Policy::Lfu => 0,
			Policy::Lru => 1,
			Policy::Mru => 2,
		}
	}
}
