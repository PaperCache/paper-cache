mod policy_stack;

use std::hash::Hash;
pub use crate::policy::policy_stack::PolicyStackType;

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Policy {
	Lfu,
	Fifo,
	Lru,
	Mru,
}

impl Policy {
	#[must_use]
	pub fn as_policy_stack_type<K>(&self) -> PolicyStackType<K>
	where
		K: Copy + Eq + Hash,
	{
		match self {
			Policy::Lfu => PolicyStackType::Lfu(Box::default()),
			Policy::Fifo => PolicyStackType::Fifo(Box::default()),
			Policy::Lru => PolicyStackType::Lru(Box::default()),
			Policy::Mru => PolicyStackType::Mru(Box::default()),
		}
	}
}

impl<K> PartialEq<PolicyStackType<K>> for Policy
where
	K: Copy + Eq + Hash,
{
	fn eq(&self, policy_type: &PolicyStackType<K>) -> bool {
		matches!(
			(self, policy_type),
			(Policy::Lfu, PolicyStackType::Lfu(_))
			| (Policy::Fifo, PolicyStackType::Fifo(_))
			| (Policy::Lru, PolicyStackType::Lru(_))
			| (Policy::Mru, PolicyStackType::Mru(_))
		)
	}
}

impl<K> PartialEq<&PolicyStackType<K>> for Policy
where
	K: Copy + Eq + Hash,
{
	fn eq(&self, policy_type: &&PolicyStackType<K>) -> bool {
		self.eq(*policy_type)
	}
}

pub use crate::policy::policy_stack::PolicyStack;
