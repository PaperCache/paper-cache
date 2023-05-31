use crate::cache::CacheSize;

#[derive(Clone, Copy)]
pub struct Stats {
	max_size: CacheSize,
	used_size: CacheSize,

	total_hits: u64,
	total_gets: u64,
}

/// This struct holds the basic statistical information about `PaperCache`.
impl Stats {
	/// Creates an empty statistics manager.
	pub fn new(max_size: CacheSize) -> Self {
		Stats {
			max_size,
			used_size: 0,

			total_hits: 0,
			total_gets: 0,
		}
	}

	/// Returns the cache's maximum size.
	///
	/// # Examples
	/// ```
	/// let stats = Stats::new(10);
	/// assert_eq!(stats.get_max_size(), 10);
	/// ```
	pub fn get_max_size(&self) -> &CacheSize {
		&self.max_size
	}

	/// Returns the cache's used size.
	///
	/// # Examples
	/// ```
	/// let mut stats = Stats::new(10);
	///
	/// // The cache is currently empty.
	/// assert_eq!(stats.get_used_size(), 0);
	///
	/// // The cache gets filled.
	/// stats.increase_used_size(10);
	/// assert_eq!(stats.get_used_size(), 10);
	/// ```
	pub fn get_used_size(&self) -> &CacheSize {
		&self.used_size
	}

	/// Returns the cache's total number of gets.
	///
	/// # Examples
	/// ```
	/// let mut stats = Stats::new(10);
	///
	/// assert_eq!(stats.get_total_gets(), 0);
	///
	/// stats.hit();
	///
	/// assert_eq!(stats.get_total_gets(), 1);
	///
	/// // The cache gets filled.
	/// stats.increase_used_size(10);
	/// assert_eq!(stats.get_used_size(), 10);
	/// ```
	pub fn get_total_gets(&self) -> &u64 {
		&self.total_gets
	}

	/// Returns the cache's current miss ratio.
	///
	/// # Examples
	/// ```
	/// let mut stats = Stats::new(10);
	///
	/// // The cache has not had any gets yet, therefore the
	/// // miss ratio should be one.
	/// assert_eq!(stats.get_miss_ratio(), 1.0);
	///
	/// // The cache gets hits.
	/// stats.hit();
	/// stats.hit();
	/// stats.hit();
	/// stats.miss();
	///
	/// assert_eq!(stats.get_miss_ratio(), 0.25);
	/// stats.
	/// ```
	pub fn get_miss_ratio(&self) -> f64 {
		if self.total_gets == 0 {
			return 1.0;
		}

		1.0 - self.total_hits as f64 / self.total_gets as f64
	}

	/// Records a cache hit and increments the total gets counter.
	///
	/// # Examples
	/// ```
	/// let mut stats = Stats::new(10);
	///
	/// assert_eq!(stats.get_total_gets(), 0);
	/// assert_eq!(stats.get_miss_ratio(), 1.0);
	///
	/// self.hit();
	///
	/// assert_eq!(stats.get_total_gets(), 1);
	/// assert_eq!(stats.get_miss_ratio(), 0.0);
	/// ```
	pub fn hit(&mut self) {
		self.total_gets += 1;
		self.total_hits += 1;
	}

	/// Records a cache miss and increments the total gets counter.
	///
	/// # Examples
	/// ```
	/// let mut stats = Stats::new(10);
	///
	/// assert_eq!(stats.get_total_gets(), 0);
	/// assert_eq!(stats.get_miss_ratio(), 1.0);
	///
	/// self.miss();
	///
	/// assert_eq!(stats.get_total_gets(), 1);
	/// assert_eq!(stats.get_miss_ratio(), 1.0);
	/// ```
	pub fn miss(&mut self) {
		self.total_gets += 1;
	}

	/// Sets the cache's maximum size.
	///
	/// # Examples
	/// ```
	/// let stats = Stats::new(10);
	/// assert_eq!(stats.get_max_size(), 10);
	///
	/// stats.set_max_size(5);
	/// assert_eq!(stats.get_max_size(), 5);
	/// ```
	pub fn set_max_size(&mut self, max_size: &u64) {
		self.max_size = *max_size;
	}

	/// Increases the cache's used size by the supplied amount.
	///
	/// # Examples
	/// ```
	/// let mut stats = Stats::new(10);
	///
	/// assert_eq!(stats.get_used_size(), 10);
	///
	/// stats.increase_used_size(5);
	/// assert_eq!(stats.get_used_size(), 15);
	/// ```
	pub fn increase_used_size(&mut self, size: &u64) {
		self.used_size += *size;
	}

	/// Decreases the cache's used size by the supplied amount.
	///
	/// # Examples
	/// ```
	/// let mut stats = Stats::new(10);
	///
	/// assert_eq!(stats.get_used_size(), 10);
	///
	/// stats.decrease_used_size(5);
	/// assert_eq!(stats.get_used_size(), 5);
	/// ```
	pub fn decrease_used_size(&mut self, size: &u64) {
		self.used_size -= *size;
	}

	/// Sets the cache's used size to zero.
	///
	/// # Examples
	/// ```
	/// let mut stats = Stats::new(10);
	///
	/// stats.increase_used_size(5);
	/// assert_eq!(stats.get_used_size(), 5);
	///
	/// stats.reset_used_size();
	/// assert_eq!(stats.get_used_size(), 0);
	/// ```
	pub fn reset_used_size(&mut self) {
		self.used_size = 0;
	}

	/// Returns true if the cache's maximum size exceeds the supplied size.
	///
	/// # Examples
	/// ```
	/// let mut stats = Stats::new(10);
	/// assert!(stats.max_size_exceeds(5));
	/// assert!(!stats.max_size_exceeds(15));
	/// ```
	pub fn max_size_exceeds(&self, size: &u64) -> bool {
		self.max_size > *size
	}

	/// Returns true if the cache's used size exceeds the supplied size.
	///
	/// # Examples
	/// ```
	/// let mut stats = Stats::new(10);
	///
	/// stats.increase_used_size(10);
	///
	/// assert!(stats.max_size_exceeds(5));
	/// assert!(!stats.max_size_exceeds(15));
	/// ```
	pub fn used_size_exceeds(&self, size: &u64) -> bool {
		self.used_size > *size
	}

	/// Returns the target used size of the cache to be able to fit the
	/// supplied size without exceeding the maximum size.
	///
	/// # Examples
	/// ```
	/// let mut stats = Stats::new(10);
	///
	/// assert!(stats.target_size_to_fit(2), 8);
	/// ```
	pub fn target_used_size_to_fit(&self, size: &u64) -> u64 {
		self.max_size - *size
	}
}
