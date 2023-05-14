use std::error::Error;
use std::fmt::{Display, Formatter};

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
pub struct PaperError {
	kind: ErrorKind,
	message: String,
}

impl PaperError {
	/// Creates a new [`PaperError`] with the supplied
	/// [`ErrorKind`] and `message`.
	pub fn new(kind: ErrorKind, message: &str) -> Self {
		PaperError {
			kind,
			message: message.to_owned(),
		}
	}

	/// Returns the [`ErrorKind`].
	pub fn kind(&self) -> &ErrorKind {
		&self.kind
	}
}

impl Error for PaperError {}

impl Display for PaperError {
	fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
		write!(f, "{}", self.message)
	}
}
