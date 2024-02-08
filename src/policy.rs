mod policy_stack;
mod policy_type;

use std::hash::Hash;
pub use crate::policy::policy_type::PolicyType;

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Policy {
	Lfu,
	Fifo,
	Lru,
	Mru,
}

impl Policy {
	#[must_use]
	pub fn as_policy_type<K>(&self) -> PolicyType<K>
	where
		K: Eq + Hash,
	{
		match self {
			Policy::Lfu => PolicyType::Lfu(Box::default()),
			Policy::Fifo => PolicyType::Fifo(Box::default()),
			Policy::Lru => PolicyType::Lru(Box::default()),
			Policy::Mru => PolicyType::Mru(Box::default()),
		}
	}
}

impl<K> PartialEq<PolicyType<K>> for Policy
where
	K: Eq + Hash,
{
	fn eq(&self, policy_type: &PolicyType<K>) -> bool {
		matches!(
			(self, policy_type),
			(Policy::Lfu, PolicyType::Lfu(_))
			| (Policy::Fifo, PolicyType::Fifo(_))
			| (Policy::Lru, PolicyType::Lru(_))
			| (Policy::Mru, PolicyType::Mru(_))
		)
	}
}

impl<K> PartialEq<&PolicyType<K>> for Policy
where
	K: Eq + Hash,
{
	fn eq(&self, policy_type: &&PolicyType<K>) -> bool {
		self.eq(*policy_type)
	}
}

pub use crate::policy::policy_stack::PolicyStack;
