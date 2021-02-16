// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cell::RefCell;
use std::rc::Rc;

use crate::Shell;
use crate::ffi::evdev::BTN_LEFT;
use crate::ffi;

mod pointer_grab {
    use super::ffi;

    extern "C" fn focus(_grab: *mut ffi::weston_pointer_grab) {
    }

    extern "C" fn motion(grab: *mut ffi::weston_pointer_grab,
			 _time: *const ffi::timespec,
			 event: *mut ffi::weston_pointer_motion_event) {
	let grab = unsafe { &mut *container_of_mut!(grab, super::MoveGrab, grab) };
	let pointer = unsafe { &mut *grab.grab.pointer };
	let view = unsafe { &mut *pointer.focus };

	unsafe {
	    ffi::weston_pointer_move(pointer, event);
	}

	println!("motion: {},{}", pointer.x, pointer.y);

	view.set_position(ffi::fixed_to_int(pointer.x + grab.dx) as f32,
			  ffi::fixed_to_int(pointer.y + grab.dy) as f32);
	    

	unsafe {
	    ffi::weston_compositor_schedule_repaint((*view.surface).compositor);
	}
    }

    extern "C" fn  button(grab: *mut ffi::weston_pointer_grab,
			  _time: *const ffi::timespec,
			  _button: u32,
			  state: u32) {
	let grab = unsafe { &mut *container_of_mut!(grab, super::MoveGrab, grab) };
	let pointer = unsafe { &mut *grab.grab.pointer };

	println!("button");
	if pointer.button_count == 0 &&
	    state == ffi::wl_pointer_button_state::WL_POINTER_BUTTON_STATE_RELEASED as u32 {
		// Put it back in the box and drop it
		let _ = unsafe { Box::from_raw(grab as *mut _) };
	    }
    }

    extern "C" fn axis(_grab: *mut ffi::weston_pointer_grab,
		       _time: *const ffi::timespec,
		       _event: *mut ffi::weston_pointer_axis_event) {
    }

    extern "C" fn  axis_source(_grab: *mut ffi::weston_pointer_grab, _source: u32) {
    }
    
    extern "C" fn  frame(_grab: *mut ffi::weston_pointer_grab) {
    }
    
    extern "C" fn cancel(_grab: *mut ffi::weston_pointer_grab) {
    }

    pub static INTERFACE: ffi::weston_pointer_grab_interface =
	ffi::weston_pointer_grab_interface {
	    focus: Some(focus),
	    motion: Some(motion),
	    button: Some(button),
	    axis: Some(axis),
	    axis_source: Some(axis_source),
	    frame: Some(frame),
	    cancel: Some(cancel),
	};
}

struct MoveGrab {
    grab: ffi::weston_pointer_grab,
    view_destroy_listener: ffi::wl_listener,
    dx: ffi::wl_fixed_t,
    dy: ffi::wl_fixed_t,
}

impl MoveGrab {
    fn start(pointer: *mut ffi::weston_pointer) {
	let pointer = unsafe { &mut *pointer };
	let view = unsafe { &mut *pointer.focus };

	let mut grab = Box::new(MoveGrab {
	    grab: ffi::weston_pointer_grab {
		interface: &pointer_grab::INTERFACE,
		pointer: pointer,
	    },
	    view_destroy_listener: ffi::wl_listener::new(view_destroy_notify),
	    dx: ffi::fixed_from_double(view.geometry.x) - pointer.grab_x,
	    dy: ffi::fixed_from_double(view.geometry.y) - pointer.grab_y,
	});

	unsafe {
	    ffi::weston_pointer_start_grab(pointer, &mut grab.grab);
	}

	// Leak and let it fly...
	Box::into_raw(grab);
    }
}

impl Drop for MoveGrab {
    fn drop(&mut self) {
	unsafe {
	    ffi::weston_pointer_end_grab(self.grab.pointer)
	}
	println!("dropped grab");
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

    compositor.bind_button(
	BTN_LEFT,
	ffi::weston_keyboard_modifier::MODIFIER_ALT,
	|pointer, _, _| {
	    println!("button binding fires!");
	    MoveGrab::start(pointer);
	});
}
