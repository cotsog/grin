// Copyright 2018 The Grin Developers
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{self, LockResult, RwLockReadGuard, RwLockWriteGuard, TryLockResult};
use std::sync::{mpsc, Arc};

use std::cell::Cell;
use std::{thread, time};

use backtrace::Backtrace;
use chrono::prelude::*;
use chrono::{Duration, Utc};

/// Observable RwLock
pub struct RwLock<T: ?Sized> {
	/// Lock tracking
	pub lock_track: Arc<sync::RwLock<Vec<(DateTime<Utc>, Backtrace, bool)>>>,
	/// Flag to indicate observing on going
	observing: Arc<AtomicBool>,
	/// Channel for stopping observing
	close_tx: Cell<Option<mpsc::Sender<()>>>,
	/// Inner RwLock from stdlib
	inner: sync::RwLock<T>,
}

unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}
unsafe impl<T: ?Sized + Send + Sync> Sync for RwLock<T> {}

impl<T: ?Sized> Drop for RwLock<T> {
	fn drop(&mut self) {
		unsafe {
			if let Some(ref tx) = *self.close_tx.as_ptr() {
				let _ = tx.send(());
			}
		}
		thread::sleep(time::Duration::from_millis(1));
	}
}

impl<T> RwLock<T> {
	/// Observable RwLock creation
	pub fn new(t: T) -> RwLock<T> {
		let the_lock = RwLock {
			inner: sync::RwLock::new(t),
			lock_track: Arc::new(sync::RwLock::new(vec![])),
			observing: Arc::new(AtomicBool::new(false)),
			close_tx: Cell::new(None),
		};

		the_lock
	}
}

impl<T: ?Sized> RwLock<T> {
	fn start_observing(&self) {
		self.observing.store(true, Ordering::Relaxed);

		let lock_track = self.lock_track.clone();
		let (close_tx, close_rx) = mpsc::channel();
		self.close_tx.set(Some(close_tx));

		let _ = thread::Builder::new()
			.name("rwlock".to_string())
			.spawn(move || {
				loop {
					if let Ok(_) = close_rx.recv_timeout(time::Duration::from_secs(300)) {
						break;
					}

					{
						let tracks = lock_track.read().unwrap();
						if tracks.len() == 0 {
							continue;
						}

						let last_call = tracks.last().clone();
						if let Some(lc) = last_call {
							let now = Utc::now();
							// log this possible dead-lock
							if !lc.2 && now > lc.0 + Duration::minutes(3) {
								parse_backtrace(lock_track.clone());
								continue;
							}
						}
					}
				}
				println!("RwLock released, thread exit.");
			});
	}

	// update lock status if lock success
	fn lock_success(&self, call_time: DateTime<Utc>) {
		let mut lock_track = self.lock_track.write().unwrap();
		let index = lock_track.iter().position(|&ref r| r.0 == call_time);

		if let Some(i) = index {
			lock_track[i].2 = true;
		}
	}

	fn backtrace_push(&self) -> DateTime<Utc> {
		let call_time = Utc::now();

		// get back trace for current call
		let bt = Backtrace::new();

		// save this call
		let mut lock_track = self.lock_track.write().unwrap();
		lock_track.push((call_time, bt, false));

		// limit the size
		let size = lock_track.len();
		if size > 20 {
			lock_track.drain(..size - 20);
		}

		return call_time;
	}

	/// Observable RwLock Read Lock
	pub fn read(&self) -> LockResult<RwLockReadGuard<T>> {
		if !self.observing.load(Ordering::Relaxed) {
			self.start_observing();
		}
		let call_time = self.backtrace_push();
		let read = self.inner.read();
		self.lock_success(call_time);
		read
	}

	/// Observable RwLock Try_Read Lock
	pub fn try_read(&self) -> TryLockResult<RwLockReadGuard<T>> {
		if !self.observing.load(Ordering::Relaxed) {
			self.start_observing();
		}
		let call_time = self.backtrace_push();
		let read = self.inner.try_read();
		if read.is_ok() {
			self.lock_success(call_time);
		}
		read
	}

	/// Observable RwLock Write Lock
	pub fn write(&self) -> LockResult<RwLockWriteGuard<T>> {
		if !self.observing.load(Ordering::Relaxed) {
			self.start_observing();
		}
		let call_time = self.backtrace_push();
		let write = self.inner.write();
		self.lock_success(call_time);
		write
	}
}

/// Util function to show the lock history of a RwLock
pub fn parse_backtrace(lock_track: Arc<sync::RwLock<Vec<(DateTime<Utc>, Backtrace, bool)>>>) {
	const NANO_TO_MILLIS: f64 = 1.0 / 1_000_000.0;

	let lock_track = lock_track.clone();
	let tracks = lock_track.read().unwrap();
	println!("\n-------------------------{}", Utc::now());
	let mut now = Utc::now().timestamp_nanos();
	let mut i = 0;
	for (call_time, one_call, locked) in tracks.iter().rev() {
		i += 1;
		let time = call_time.timestamp_nanos();
		let dur_ms = (now - time) as f64 * NANO_TO_MILLIS;
		println!(
			"\n#{} - call time: {:?}, locked: {}, delta: {:.3?}ms\n",
			i, call_time, locked, dur_ms
		);
		println!("{:?}", one_call);
		now = time;
	}
}
