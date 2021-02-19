// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::ptr::NonNull;
use crate::ffi;

pub trait PointerGrabHandler {
    fn focus(&mut self,
	     _pointer: &mut ffi::weston_pointer) {
    }

    fn motion(&mut self,
	      _pointer: &mut ffi::weston_pointer,
	      _time: &ffi::timespec,
	      _event: *mut ffi::weston_pointer_motion_event) { }

    fn button(&mut self,
	      _pointer: &mut ffi::weston_pointer,
	      _time: &ffi::timespec,
	      _button: u32,
	      _state: u32) { }

    fn axis(&mut self,
	    _pointer: &mut ffi::weston_pointer,
	    _time: *const ffi::timespec,
	    _event: *mut ffi::weston_pointer_axis_event) { }

    fn axis_source(&mut self,
		   _pointer: &mut ffi::weston_pointer, _source: u32) { }
    
    fn frame(&mut self,
	     _pointer: &mut ffi::weston_pointer) { }

    fn cancel(&mut self,
	      _pointer: &mut ffi::weston_pointer) { }
}

pub struct PointerGrab {
    grab: ffi::weston_pointer_grab,
    handler: Option<NonNull<dyn PointerGrabHandler>>
}

impl PointerGrab {

    fn cast(grab: *mut ffi::weston_pointer_grab) ->
	(&'static mut ffi::weston_pointer, &'static mut dyn PointerGrabHandler) {
	let grab = unsafe { &mut *container_of_mut!(grab, PointerGrab, grab) };
	unsafe { (&mut *grab.grab.pointer,  &mut *grab.handler.unwrap().as_ptr()) }
    }

    extern "C" fn focus(grab: *mut ffi::weston_pointer_grab) {
	let (pointer, handler) = Self::cast(grab);

	handler.focus(pointer);
    }

    extern "C" fn motion(grab: *mut ffi::weston_pointer_grab,
			 time: *const ffi::timespec,
			 event: *mut ffi::weston_pointer_motion_event) {
	let (pointer, handler) = Self::cast(grab);
	let time = unsafe { &*time };

	handler.motion(pointer, time, event);
    }

    extern "C" fn button(grab: *mut ffi::weston_pointer_grab,
					 time: *const ffi::timespec,
					 button: u32,
					 state: u32) {
	let (pointer, handler) = Self::cast(grab);
	let time = unsafe { &*time };

	handler.button(pointer, time, button, state)
    }

    extern "C" fn axis(grab: *mut ffi::weston_pointer_grab,
				       time: *const ffi::timespec,
				       event: *mut ffi::weston_pointer_axis_event) {
	let (pointer, handler) = Self::cast(grab);
	let time = unsafe { &*time };
	let event = unsafe { &mut *event };

	handler.axis(pointer, time, event)
    }

    extern "C" fn axis_source(grab: *mut ffi::weston_pointer_grab, source: u32) {
	let (pointer, handler) = Self::cast(grab);

	handler.axis_source(pointer, source)
    }
    
    extern "C" fn frame(grab: *mut ffi::weston_pointer_grab) {
	let (pointer, handler) = Self::cast(grab);

	handler.frame(pointer)
    }
    
    extern "C" fn cancel(grab: *mut ffi::weston_pointer_grab) {
	let (pointer, handler) = Self::cast(grab);

	handler.cancel(pointer);
    }

    pub fn new(pointer: *mut ffi::weston_pointer) -> PointerGrab {
	static INTERFACE: ffi::weston_pointer_grab_interface =
	    ffi::weston_pointer_grab_interface {
		focus: Some(PointerGrab::focus),
		motion: Some(PointerGrab::motion),
		button: Some(PointerGrab::button),
		axis: Some(PointerGrab::axis),
		axis_source: Some(PointerGrab::axis_source),
		frame: Some(PointerGrab::frame),
		cancel: Some(PointerGrab::cancel),
	    };

	PointerGrab {
	    grab: ffi::weston_pointer_grab {
		interface: &INTERFACE,
		pointer: pointer,
	    },
	    handler: None
	}
    }

    pub fn start(&mut self, handler: *mut dyn PointerGrabHandler) {
	// This is a separate function since it needs a stable address for self
	self.handler = NonNull::new(handler);
	unsafe {
	    ffi::weston_pointer_start_grab(self.grab.pointer, &mut self.grab)
	};
    }

}

impl Drop for PointerGrab {
    fn drop(&mut self) {
	unsafe {
	    ffi::weston_pointer_end_grab(self.grab.pointer)
	}
    }
}
