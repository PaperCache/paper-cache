#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Policy {
	Lfu,
	Fifo,
	Lru,
	Mru,
}

impl Policy {
	pub fn index(&self) -> usize {
		match self {
			Policy::Lfu => 0,
			Policy::Fifo => 1,
			Policy::Lru => 2,
			Policy::Mru => 3,
		}
	}
}
