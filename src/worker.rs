mod ttl;

pub const TIME_INCREMENT: u64 = 500;

pub use crate::worker::ttl::worker as ttl_worker;
