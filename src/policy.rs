mod policy_stack;

use std::hash::Hash;
pub use crate::policy::policy_stack::PolicyStackType;

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum PaperPolicy {
	Lfu,
	Fifo,
	Lru,
	Mru,
}

impl PaperPolicy {
	#[must_use]
	pub fn as_policy_stack_type<K>(&self) -> PolicyStackType<K>
	where
		K: Copy + Eq + Hash,
	{
		match self {
			PaperPolicy::Lfu => PolicyStackType::Lfu(Box::default()),
			PaperPolicy::Fifo => PolicyStackType::Fifo(Box::default()),
			PaperPolicy::Lru => PolicyStackType::Lru(Box::default()),
			PaperPolicy::Mru => PolicyStackType::Mru(Box::default()),
		}
	}
}

impl<K> PartialEq<PolicyStackType<K>> for PaperPolicy
where
	K: Copy + Eq + Hash,
{
	fn eq(&self, policy_type: &PolicyStackType<K>) -> bool {
		matches!(
			(self, policy_type),
			(PaperPolicy::Lfu, PolicyStackType::Lfu(_))
			| (PaperPolicy::Fifo, PolicyStackType::Fifo(_))
			| (PaperPolicy::Lru, PolicyStackType::Lru(_))
			| (PaperPolicy::Mru, PolicyStackType::Mru(_))
		)
	}
}

impl<K> PartialEq<&PolicyStackType<K>> for PaperPolicy
where
	K: Copy + Eq + Hash,
{
	fn eq(&self, policy_type: &&PolicyStackType<K>) -> bool {
		self.eq(*policy_type)
	}
}

pub use crate::policy::policy_stack::PolicyStack;
