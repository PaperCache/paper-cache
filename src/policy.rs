#[derive(PartialEq, Clone, Copy, Debug)]
pub enum PaperPolicy {
	Lfu,
	Fifo,
	Lru,
	Mru,
}
