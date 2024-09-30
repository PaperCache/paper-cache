mod policy_stack;
mod mini_stack;

use std::hash::Hash;
pub use crate::policy::policy_stack::PolicyStackType;

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum PaperPolicy {
	Lfu,
	Fifo,
	Lru,
	Mru,
}

impl<K> From<PaperPolicy> for PolicyStackType<K>
where
	K: Copy + Eq + Hash,
{
	fn from(policy: PaperPolicy) -> Self {
		(&policy).into()
	}
}

impl<K> From<&PaperPolicy> for PolicyStackType<K>
where
	K: Copy + Eq + Hash,
{
	fn from(policy: &PaperPolicy) -> Self {
		match policy {
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
		self.eq(&policy_type)
	}
}

impl<K> PartialEq<&PolicyStackType<K>> for PaperPolicy
where
	K: Copy + Eq + Hash,
{
	fn eq(&self, policy_type: &&PolicyStackType<K>) -> bool {
		matches!(
			(self, policy_type),
			(PaperPolicy::Lfu, PolicyStackType::Lfu(_))
			| (PaperPolicy::Fifo, PolicyStackType::Fifo(_))
			| (PaperPolicy::Lru, PolicyStackType::Lru(_))
			| (PaperPolicy::Mru, PolicyStackType::Mru(_))
		)
	}
}

pub use crate::policy::{
	policy_stack::PolicyStack,
	mini_stack::MiniStackType,
};
