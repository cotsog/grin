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

extern crate grin_util as util;

use util::rwlock::*;

fn a() {
	b();
}

fn b() {
	c();
}

fn c() {
	d();
}

fn d() {
	let lock = RwLock::new(6);
	let mut w = lock.write().unwrap();
	*w -= 1;
	assert_eq!(*w, 5);
	parse_backtrace(lock.lock_track.clone());
}

#[test]
fn test_rwlock_observing() {
	let lock = RwLock::new(6);

	{
		let mut w = lock.write().unwrap();
		*w -= 1;
		assert_eq!(*w, 5);
	}

	{
		let r1 = lock.read().unwrap();
		let r2 = lock.read().unwrap();
		assert_eq!(*r1, 5);
		assert_eq!(*r2, 5);
	}

	{
		let mut w = lock.write().unwrap();
		*w += 1;
		assert_eq!(*w, 6);
	}

	a();
}
