use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
	#[error("The policies cannot be empty.")]
	EmptyPolicies,

	#[error("The supplied policy is not one of the cache's configured policies.")]
	UnconfiguredPolicy,

	#[error("The key was not found in the cache.")]
	KeyNotFound,

	#[error("The value size cannot be zero.")]
	ZeroValueSize,

	#[error("The value size cannot exceed the cache size.")]
	ExceedingValueSize,

	#[error("The cache size cannot be zero.")]
	ZeroCacheSize,

	#[error("Internal error.")]
	Internal,
}
