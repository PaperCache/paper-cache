use thiserror::Error;

#[derive(Debug, PartialEq, Error)]
pub enum CacheError {
	#[error("internal error")]
	Internal,

	#[error("the key was not found in the cache")]
	KeyNotFound,

	#[error("the value size cannot be zero")]
	ZeroValueSize,

	#[error("the value size cannot exceed the cache size")]
	ExceedingValueSize,

	#[error("the cache size cannot be zero")]
	ZeroCacheSize,
}
