use crate::paper_cache::CacheSize;

pub enum Command<K, V> {
	Get(K),
	Set(K, V),
	Del(K),
	Resize(CacheSize),
}
