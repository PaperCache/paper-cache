use std::{
	error::Error,
	fmt::{Display, Formatter},
};

pub use paper_utils::error::PaperError;

#[derive(Debug)]
pub enum ErrorKind {
	InvalidPolicies,
	InvalidPolicy,
	KeyNotFound,
	InvalidValueSize,
	InvalidCacheSize,
	Internal,
}

#[derive(Debug)]
pub struct CacheError {
	kind: ErrorKind,
	message: String,
}

impl CacheError {
	/// Creates a new [`CacheError`] with the supplied
	/// [`ErrorKind`] and `message`.
	pub fn new(kind: ErrorKind, message: &str) -> Self {
		CacheError {
			kind,
			message: message.to_owned(),
		}
	}

	/// Returns the [`ErrorKind`].
	pub fn kind(&self) -> &ErrorKind {
		&self.kind
	}
}

impl PaperError for CacheError {
	/// Returns the `message`
	fn message(&self) -> &str {
		&self.message
	}
}

impl Error for CacheError {}

impl Display for CacheError {
	fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
		write!(f, "{}", self.message)
	}
}
