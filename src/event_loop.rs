// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::os::unix::io::{FromRawFd, IntoRawFd, RawFd};
use std::os::unix::net::{SocketAddr, UnixListener};

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

    pub fn on_ready<F>(&mut self, fd: RawFd, mask: ffi::wl_event_mask, f: F)
    where
        F: Fn(RawFd),
    {
        extern "C" fn call_closure<F>(fd: RawFd, _: u32, data: *mut ::std::os::raw::c_void) -> i32
        where
            F: Fn(RawFd),
        {
            let f = unsafe { &*(data as *mut F) };
            f(fd);

            1
        }

        let p = Box::into_raw(Box::new(f));

        unsafe {
            ffi::wl_event_loop_add_fd(
                self.native,
                fd,
                mask.0,
                Some(call_closure::<F>),
                p as *mut _,
            );
        }
    }

    pub fn on_connect<F>(&mut self, listener: UnixListener, f: F)
    where
        F: Fn(RawFd, SocketAddr),
    {
        self.on_ready(listener.into_raw_fd(), ffi::wl_event_mask::WL_EVENT_READABLE, move |fd| {
            let listener = unsafe { UnixListener::from_raw_fd(fd) };

            match listener.accept() {
                Ok((socket, addr)) => f(socket.into_raw_fd(), addr),
                Err(_) => (),
            }

            listener.into_raw_fd();
        });
    }
}
