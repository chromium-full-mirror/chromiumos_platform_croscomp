// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cell::RefCell;
use std::rc::Rc;

use crate::Shell;
//use crate::input;
use crate::ffi;
use crate::ffi::evdev::{BTN_LEFT, KEY_F3};

// TODO:
//
//  - Rotate on Ctrl-Alt-Shift-REFRESH
//

extern "C" fn view_destroy_notify(_listener: *mut ffi::wl_listener,
				  _data: *mut ::std::os::raw::c_void) {
    println!("view_destroy_notify");
}
    

struct Rotation {
    rotation: ffi::weston_matrix,
    transform: ffi::weston_transform,
    view_destroy_listener: ffi::wl_listener
}

fn key_rotate(surface: &mut ffi::weston_surface) {
    let mut rotation = Box::new(Rotation {
	rotation: ffi::weston_matrix::new(),
	transform: unsafe { std::mem::zeroed() },
	view_destroy_listener: ffi::wl_listener::new(view_destroy_notify)
    });
			    
    let view = unsafe { &mut *ffi::get_default_view(surface as *mut _) };

    view.destroy_signal.add(&mut rotation.view_destroy_listener);

    println!("rotate {:?}", surface as *mut _)
}

fn pointer_rotate(view: &mut ffi::weston_view) {
    println!("pointer rotate {:?}", view as *mut _)
}

pub fn init(shell: &Rc<RefCell<Shell>>) {
    let shell = shell.borrow();
    let mut compositor = shell.compositor.borrow_mut();

    let mods = ffi::weston_keyboard_modifier::MODIFIER_CTRL
        | ffi::weston_keyboard_modifier::MODIFIER_ALT
        | ffi::weston_keyboard_modifier::MODIFIER_SHIFT;

    compositor.bind_key(KEY_F3, mods, move |keyboard, _, _| {
        let focus = unsafe { &mut *(*keyboard).focus };

        key_rotate(focus)
    });

    compositor.bind_button(BTN_LEFT, mods, |pointer: *mut ffi::weston_pointer, _, _| {
        let focus = unsafe { &mut *(*pointer).focus };

        pointer_rotate(focus)
    });
}
