/*
 * Copyright (c) Kia Shakiba
 *
 * This source code is licensed under the GNU AGPLv3 license found in the
 * LICENSE file in the root directory of this source tree.
 */

mod fragment;

use std::{
	thread,
	sync::Arc,
	time::Duration,
	collections::VecDeque,
};

use parking_lot::RwLock;
use crossbeam_channel::Receiver;
use kwik::file::FileWriter;

use crate::{
	error::CacheError,
	worker::{
		Worker,
		policy::event::{StackEvent, TraceEvent},
	},
};

pub use crate::worker::policy::trace::fragment::TraceFragment;

const POLL_DELAY: Duration = Duration::from_secs(1);

pub struct TraceWorker {
	listener: Receiver<StackEvent>,
	trace_fragments: Arc<RwLock<VecDeque<TraceFragment>>>,
}

impl Worker for TraceWorker {
	fn run(&mut self) -> Result<(), CacheError> {
		loop {
			let events = self.listener
				.try_iter()
				.collect::<Vec<_>>();

			if !events.is_empty() {
				self.refresh_fragments()?;
				let mut should_flush = false;

				for event in events {
					if matches!(event, StackEvent::Wipe) {
						// wiping the cache deletes all the trace fragments
						self.trace_fragments.write().clear();
						self.refresh_fragments()?;
					}

					if let Some(event) = TraceEvent::maybe_from_stack_event(&event) {
						let fragments = self.trace_fragments.read();
						let fragment = fragments.back().ok_or(CacheError::Internal)?;

						let mut modifiers = fragment.lock();
						let writer = &mut modifiers.1;

						writer
							.write_chunk(&event)
							.map_err(|_| CacheError::Internal)?;

						should_flush = true;
					}
				}

				if should_flush {
					let fragments = self.trace_fragments.read();
					let fragment = fragments.back().ok_or(CacheError::Internal)?;

					let mut modifiers = fragment.lock();
					let writer = &mut modifiers.1;

					writer
						.flush()
						.map_err(|_| CacheError::Internal)?;
				}
			}

			thread::sleep(POLL_DELAY);
		}
	}
}

impl TraceWorker {
	pub fn new(
		listener: Receiver<StackEvent>,
		trace_fragments: Arc<RwLock<VecDeque<TraceFragment>>>,
	) -> Self {
		TraceWorker {
			listener,
			trace_fragments,
		}
	}

	/// Ensures all trace fragments are younger than TRACE_MAX_AGE and the
	/// youngest fragment is also younger than TRACE_REFRESH_AGE
	fn refresh_fragments(&mut self) -> Result<(), CacheError> {
		// remove any fragments that are expired
		while self.trace_fragments
			.read()
			.front()
			.is_some_and(|fragment| fragment.is_expired()) {

			self.trace_fragments.write().pop_front();
		}

		if self.trace_fragments
			.read()
			.back()
			.is_some_and(|fragment| fragment.is_valid()) {

			// the latest trace is still valid
			return Ok(());
		}

		// the latest fragment is no longer valid, so create a new one
		let fragment = TraceFragment::new()
			.map_err(|_| CacheError::Internal)?;

		self.trace_fragments
			.write()
			.push_back(fragment);

		Ok(())
	}
}

unsafe impl Send for TraceWorker {}
