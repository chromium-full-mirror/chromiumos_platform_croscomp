// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::ffi;

#[derive(Debug)]
pub struct EventLoop {
    native: *mut ffi::wl_event_loop,
}

impl EventLoop {
    pub fn new(native: *mut ffi::wl_event_loop) -> Self {
        Self { native }
    }

    pub fn when_idle<F>(&mut self, f: F)
    where
        F: FnOnce(),
    {
        extern "C" fn call_closure<F>(data: *mut ::std::os::raw::c_void)
        where
            F: FnOnce(),
        {
            let f = unsafe { Box::from_raw(data as *mut F) };

            f()
        }

        let p = Box::into_raw(Box::new(f));

        unsafe {
            ffi::wl_event_loop_add_idle(self.native, Some(call_closure::<F>), p as *mut _);
        }
    }
}
