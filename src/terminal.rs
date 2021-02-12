// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cell::RefCell;
use std::rc::Rc;
use crate::Shell;

use crate::ffi;
use crate::ffi::evdev::{KEY_F1, KEY_F2};

struct Terminal {
    client_surface: Option<*mut ffi::weston_surface>,

    shell: Rc<RefCell<crate::Shell>>,

    client: Option<*const ffi::wl_client>,

    surface_create_listener: ffi::wl_listener,
    client_destroy_listener: ffi::wl_listener,
}

type Void = ::std::os::raw::c_void;

extern "C"
fn client_destroy_notify(listener: *mut ffi::wl_listener,
			 _data: *mut ::std::os::raw::c_void) {
    let mut t = unsafe {
	&mut *container_of_mut!(listener, Terminal, client_destroy_listener)
    };

    t.client = None;
    t.client_surface = None;
}

extern "C"
fn binding(keyboard: *mut ffi::weston_keyboard,
           _time: *const ffi::timespec,
           key: u32,
           data: *mut Void) {
    let mut t = unsafe { &mut *(data as *mut Terminal) };
    let shell = &mut t.shell.borrow_mut().native;

    match (key, t.client_surface) {
	(KEY_F1, Some(surface)) => {
	    unsafe {
		let ds = ffi::weston_surface_get_desktop_surface(surface);
		ffi::weston_desktop_api_fullscreen_requested(shell, ds, false,
							     0 as *mut ffi::weston_output);
		ffi::weston_desktop_api_minimized_requested(shell, ds);
	    }
	},
	(KEY_F2, Some(surface)) => {
	    unsafe {
		let ds = ffi::weston_surface_get_desktop_surface(surface);
		let view = ffi::get_default_view(surface);
		ffi::activate(
		    shell, view, (*keyboard).seat,
		    ffi::weston_activate_flag::WESTON_ACTIVATE_FLAG_CONFIGURE as u32);
			 
		ffi::weston_desktop_api_fullscreen_requested(shell, ds,
							     true, (*surface).output);
	    }
	},
	(KEY_F2, None) => {
	    if t.client == None {
		let terminal_client_path = "/usr/bin/croscomp-terminal\0";

		let client = unsafe {
		    ffi::weston_client_start((*shell).compositor,
					     terminal_client_path.as_ptr() as *const i8)
		    // add destroy listener
		};

		t.client = Some(client);
		unsafe {
		    ffi::wl_client_add_destroy_listener(
			client,
			&mut t.client_destroy_listener as *mut ffi::wl_listener);
		}
	    }
	}
	_ => ()
    }
}

extern "C"
fn surface_create_notify(listener: *mut ffi::wl_listener,
			 data: *mut ::std::os::raw::c_void) {
    let mut t = unsafe {
	&mut *container_of_mut!(listener, Terminal, surface_create_listener)
    };
    let surface = data as *mut ffi::weston_surface;
    let client = unsafe {
	ffi::wl_resource_get_client((*surface).resource)
    };

    if t.client == Some(client) {
	println!(" -> match");
	t.client_surface = Some(surface);
    }
}

pub fn init(shell: &Rc<RefCell<Shell>>) {
    let mut t = Box::new(Terminal {
	client_surface: None,
	shell: Rc::clone(shell),
	client: None,
	surface_create_listener: ffi::wl_listener::new(surface_create_notify),
	client_destroy_listener: ffi::wl_listener::new(client_destroy_notify),
    });
    
    let s = shell.borrow();
    let mut compositor = s.compositor.borrow_mut();

    // signal add
    unsafe {
	ffi::wl_list_insert(compositor.native.create_surface_signal.listener_list.prev,
			    &mut t.surface_create_listener.link as *mut ffi::wl_list);
    }

    let p = Box::into_raw(t);

    compositor.add_key_binding(KEY_F1,
			       ffi::weston_keyboard_modifier::MODIFIER_CTRL |
			       ffi::weston_keyboard_modifier::MODIFIER_ALT,
			       Some(binding), p as *mut Void);
    compositor.add_key_binding(KEY_F2,
			       ffi::weston_keyboard_modifier::MODIFIER_CTRL |
			       ffi::weston_keyboard_modifier::MODIFIER_ALT,
			       Some(binding), p as *mut Void);

    println!("init_terminal");
}
