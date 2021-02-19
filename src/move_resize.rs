// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cell::RefCell;
use std::rc::Rc;

use crate::Shell;
use crate::input;
use crate::ffi;

struct MoveGrab {
    view_destroy_listener: ffi::wl_listener,
    dx: ffi::wl_fixed_t,
    dy: ffi::wl_fixed_t,
    grab: input::PointerGrab
}

impl input::PointerGrabHandler for MoveGrab {
    fn motion(&mut self,
	      pointer: &mut ffi::weston_pointer,
	      _time: &ffi::timespec,
	      event: *mut ffi::weston_pointer_motion_event) {
	let view = unsafe { &mut *(*pointer).focus };

	unsafe {
	    ffi::weston_pointer_move(pointer, event);
	}

	view.set_position(ffi::fixed_to_int(pointer.x + self.dx) as f32,
			  ffi::fixed_to_int(pointer.y + self.dy) as f32);

	unsafe {
	    ffi::weston_compositor_schedule_repaint((*view.surface).compositor);
	}
    }

    fn button(&mut self,
	      pointer: &mut ffi::weston_pointer,
	      _time: &ffi::timespec,
	      _button: u32,
	      state: u32) {
	if pointer.button_count == 0 &&
	    state == ffi::wl_pointer_button_state::WL_POINTER_BUTTON_STATE_RELEASED as u32 {
		// Put it back in the box and drop it
		let _ = unsafe { Box::from_raw(self as *mut _) };
	    }
    }

    fn cancel(&mut self,
	      _pointer: &mut ffi::weston_pointer) {
	let _ = unsafe { Box::from_raw(self as *mut _) };
    }
}

impl MoveGrab {
    fn start(pointer: *mut ffi::weston_pointer, view: *mut ffi::weston_view) {
	let pointer = unsafe { &mut *pointer };
	let view = unsafe { &mut *view };

	let mut handler = Box::new(MoveGrab {
	    view_destroy_listener: ffi::wl_listener::new(view_destroy_notify),
	    dx: ffi::fixed_from_double(view.geometry.x) - pointer.grab_x,
	    dy: ffi::fixed_from_double(view.geometry.y) - pointer.grab_y,
	    grab: input::PointerGrab::new(pointer)
	});

	let p = handler.as_mut() as *mut _;
	handler.grab.start(p);
	Box::into_raw(handler);
    }
}

extern "C"
fn view_destroy_notify(listener: *mut ffi::wl_listener,
		       _data: *mut ::std::os::raw::c_void) {
    let _grab = unsafe {
	&mut *container_of_mut!(listener, MoveGrab, view_destroy_listener)
    };
}

pub fn init(shell: &Rc<RefCell<Shell>>) {
    let shell = shell.borrow();
    let mut compositor = shell.compositor.borrow_mut();

    use crate::ffi::evdev::BTN_LEFT;

    compositor.bind_button(
	BTN_LEFT,
	ffi::weston_keyboard_modifier(shell.native.binding_modifier),
	|pointer, _, _| {
	    let pointer = unsafe { &mut *pointer };
	    if pointer.focus != 0 as *mut _ {
		MoveGrab::start(pointer, pointer.focus);
	    }
	});
}

#[no_mangle]
pub extern "C"
fn move_view(view: *mut ffi::weston_view,
	     pointer: *mut ffi::weston_pointer) {
    MoveGrab::start(pointer, view);    
}

