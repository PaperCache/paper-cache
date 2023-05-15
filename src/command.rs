use crate::paper_cache::CacheSize;

pub enum Command<K, V> {
	Get(K),
	Set(K, V, Option<u32>),
	Del(K),
	Resize(CacheSize),
}
