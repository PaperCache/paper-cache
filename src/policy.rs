#[derive(PartialEq, Clone, Copy)]
pub enum Policy {
	Lru,
	Mru,
}

impl Policy {
	pub fn index(&self) -> usize {
		match self {
			Policy::Lru => 0,
			Policy::Mru => 1,
		}
	}

	pub fn id(&self) -> String {
		match self {
			Policy::Lru => "lru".to_owned(),
			Policy::Mru => "mru".to_owned(),
		}
	}
}
