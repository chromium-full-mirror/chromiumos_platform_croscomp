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

    pub fn on_signal<F>(&mut self, signal: i32, f: F)
    where
        F: Fn(i32),
    {
        extern "C" fn call_closure<F>(signal: i32, data: *mut ::std::os::raw::c_void) -> i32
        where
            F: Fn(i32),
        {
            let f = unsafe { &*(data as *mut F) };

            f(signal);

            1
        }

        let p = Box::into_raw(Box::new(f));

        unsafe {
            ffi::wl_event_loop_add_signal(
                self.native,
                signal,
                Some(call_closure::<F>),
                p as *mut _,
            );
        }
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

    pub fn on_connect<F>(&mut self, listener: UnixListener, f: F)
    where
        F: Fn(RawFd, SocketAddr),
    {
        extern "C" fn call_closure<F>(fd: i32, _: u32, data: *mut ::std::os::raw::c_void) -> i32
        where
            F: Fn(RawFd, SocketAddr),
        {
            let listener = unsafe { UnixListener::from_raw_fd(fd) };
            let f = unsafe { &*(data as *mut F) };
            match listener.accept() {
                Ok((socket, addr)) => f(socket.into_raw_fd(), addr),
                Err(_) => (),
            }

            listener.into_raw_fd();

            1
        }

        let p = Box::into_raw(Box::new(f));

        unsafe {
            ffi::wl_event_loop_add_fd(
                self.native,
                listener.into_raw_fd(),
                ffi::WL_EVENT_READABLE as u32,
                Some(call_closure::<F>),
                p as *mut _,
            );
        }
    }
}
