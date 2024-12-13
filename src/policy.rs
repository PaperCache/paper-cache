#[derive(PartialEq, Clone, Copy, Debug)]
pub enum PaperPolicy {
	Lfu,
	Fifo,
	Lru,
	Mru,
}

impl PaperPolicy {
	pub fn label(&self) -> &str {
		match self {
			PaperPolicy::Lfu => "LFU",
			PaperPolicy::Fifo => "FIFO",
			PaperPolicy::Lru => "LRU",
			PaperPolicy::Mru => "MRU",
		}
	}
}
