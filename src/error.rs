use thiserror::Error;

#[derive(Debug, PartialEq, Error)]
pub enum CacheError {
	#[error("internal error")]
	Internal,

	#[error("at least one policy must be configured")]
	EmptyPolicies,

	#[error("duplicate policies were configured")]
	DuplicatePolicies,

	#[error("the supplied policy is not one of the cache's configured policies")]
	UnconfiguredPolicy,

	#[error("the key was not found in the cache")]
	KeyNotFound,

	#[error("the value size cannot be zero")]
	ZeroValueSize,

	#[error("the value size cannot exceed the cache size")]
	ExceedingValueSize,

	#[error("the cache size cannot be zero")]
	ZeroCacheSize,
}
