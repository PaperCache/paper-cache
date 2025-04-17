use std::{
	fmt::{self, Display},
	str::FromStr,
};

use serde::{
	Deserialize,
	de::{self, Deserializer, Visitor},
};

use crate::error::CacheError;

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum PaperPolicy {
	Auto,
	Lfu,
	Fifo,
	Clock,
	Lru,
	Mru,
	TwoQ(f64, f64),
	SThreeFifo(f64),
}

impl PaperPolicy {
	pub fn is_auto(&self) -> bool {
		matches!(self, PaperPolicy::Auto)
	}
}

impl Display for PaperPolicy {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			PaperPolicy::Auto => write!(f, "auto"),
			PaperPolicy::Lfu => write!(f, "lfu"),
			PaperPolicy::Fifo => write!(f, "fifo"),
			PaperPolicy::Clock => write!(f, "clock"),
			PaperPolicy::Lru => write!(f, "lru"),
			PaperPolicy::Mru => write!(f, "mru"),
			PaperPolicy::TwoQ(k_in, k_out) => write!(f, "2q-{k_in}-{k_out}"),
			PaperPolicy::SThreeFifo(ratio) => write!(f, "s3-fifo-{ratio}"),
		}
	}
}

impl FromStr for PaperPolicy {
	type Err = CacheError;

	fn from_str(value: &str) -> Result<Self, Self::Err> {
		let policy = match value {
			"auto" => PaperPolicy::Auto,

			"lfu" => PaperPolicy::Lfu,
			"fifo" => PaperPolicy::Fifo,
			"clock" => PaperPolicy::Clock,
			"lru" => PaperPolicy::Lru,
			"mru" => PaperPolicy::Mru,

			value if value.starts_with("2q-") => parse_two_q(value)?,
			value if value.starts_with("s3-fifo-") => parse_s_three_fifo(value)?,

			_ => return Err(CacheError::InvalidPolicy),
		};

		Ok(policy)
	}
}

impl<'a> Deserialize<'a> for PaperPolicy {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'a>,
	{
		deserializer.deserialize_str(PaperPolicyVisitor)
	}
}

struct PaperPolicyVisitor;

impl Visitor<'_> for PaperPolicyVisitor {
	type Value = PaperPolicy;

	fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		formatter.write_str("a PaperPolicy config")
	}

	fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
	where
		E: de::Error,
	{
		PaperPolicy::from_str(value)
			.map_err(|err| E::custom(err.to_string()))
	}
}

fn parse_two_q(value: &str) -> Result<PaperPolicy, CacheError> {
	// skip the "2q-"
	let tokens = value[3..]
		.split('-')
		.collect::<Vec<&str>>();

	if tokens.len() != 2 {
		return Err(CacheError::InvalidPolicy);
	}

	let Ok(k_in) = tokens[0].parse::<f64>() else {
		return Err(CacheError::InvalidPolicy);
	};

	let Ok(k_out) = tokens[1].parse::<f64>() else {
		return Err(CacheError::InvalidPolicy);
	};

	if k_in + k_out > 1.0
		|| !(0.0..=1.0).contains(&k_in)
		|| !(0.0..=1.0).contains(&k_out)
	{
		return Err(CacheError::InvalidPolicy);
	}

	Ok(PaperPolicy::TwoQ(k_in, k_out))
}

fn parse_s_three_fifo(value: &str) -> Result<PaperPolicy, CacheError> {
	// skip the "s3-fifo-"
	let tokens = value[8..]
		.split('-')
		.collect::<Vec<&str>>();

	if tokens.len() != 1 {
		return Err(CacheError::InvalidPolicy);
	}

	let Ok(ratio) = tokens[0].parse::<f64>() else {
		return Err(CacheError::InvalidPolicy);
	};

	if !(0.0..=1.0).contains(&ratio) {
		return Err(CacheError::InvalidPolicy);
	}

	Ok(PaperPolicy::SThreeFifo(ratio))
}
