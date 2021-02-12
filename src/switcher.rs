// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cell::RefCell;
use std::rc::Rc;

use crate::Shell;
use crate::ffi;
use crate::ffi::evdev::KEY_TAB;
use crate::Void;

struct Switcher {
    shell: Rc<RefCell<crate::Shell>>,
    grab: ffi::weston_keyboard_grab,
    current: Option<*mut ffi::weston_view>,
    view_destroy_listener: ffi::wl_listener
}

impl Switcher {
    fn start(&mut self, keyboard: *mut ffi::weston_keyboard) {
	let keyboard = &mut unsafe { *keyboard };
	unsafe {
	    ffi::lower_fullscreen_layer(&mut self.shell.borrow_mut().native, 0 as *mut _);

	    keyboard.start_grab(&mut self.grab as *mut _);
	    keyboard.set_focus(None);
	}

	self.pick(true);
    }

    fn pick(&mut self, _forwards: bool) {
	let seat = unsafe { &mut *(*self.grab.keyboard).seat };

	if seat.modifier_state.is_set(
	    ffi::weston_keyboard_modifier::MODIFIER_SHIFT) {
	    println!("prev");
	} else {
	    println!("next");
	}

	let mut shell = self.shell.borrow_mut();
	let workspace = unsafe { &mut *ffi::get_current_workspace(&mut shell.native) };

	let mut iter = wl_list_iter_mut!(&mut workspace.layer.view_list.link,
					 ffi::weston_view, layer_link.link)
	    .filter(|v| unsafe { !ffi::get_shell_surface(v.surface).is_null() });

	let next_iter = iter.clone().cycle().skip(1);

	self.current =
	    if self.current.is_none()  {
		iter.next().map(|v| v as *mut _)
	    } else {
		iter.zip(next_iter).find_map(
		    |(view, next)|
		    if self.current == Some(view as *mut _) {
			Some(next as *mut _)
		    } else {
			None
		    })
	    };

	for view in wl_list_iter_mut!(&mut workspace.layer.view_list.link,
				      ffi::weston_view, layer_link.link) {
	    view.alpha = 0.25;
	    unsafe {
		ffi::weston_view_geometry_dirty(view);
		ffi::weston_surface_damage(view.surface);
	    }
	}

	println!("new current: {:?}", self.current);

	let current = unsafe { &mut (*self.current.unwrap()) };
	let surface = unsafe { &mut (*current.surface) };
	for view in wl_list_iter_mut!(&mut surface.views,
				      ffi::weston_view, surface_link) {
	    view.alpha = 1.0;
	}

	unsafe {
	    ffi::wl_list_remove(&mut self.view_destroy_listener.link);
	    current.destroy_signal.add(&mut self.view_destroy_listener);
	}
    }

    fn shutdown(&mut self) {
	let mut shell = self.shell.borrow_mut();
	let seat = unsafe { &mut *(*self.grab.keyboard).seat };
	let workspace = unsafe { &mut *ffi::get_current_workspace(&mut shell.native) };

	unsafe { ffi::weston_keyboard_end_grab(self.grab.keyboard) };

	for view in wl_list_iter_mut!(&mut workspace.layer.view_list.link,
				      ffi::weston_view, layer_link.link) {
	    view.alpha = 1.0;
	    view.geometry_dirty();

	    unsafe {
		ffi::weston_surface_damage(view.surface);
	    }
	}

	if let Some(view) = self.current {
	    unsafe {
		ffi::activate(&mut shell.native, view, seat,
			      ffi::weston_activate_flag::WESTON_ACTIVATE_FLAG_CONFIGURE as u32);
	    }
	}

	println!("shutdown");
    }
}

mod grab {
    use super::ffi;
    use ffi::evdev::KEY_TAB;
    use ffi::wl_keyboard_key_state;

    pub extern "C" fn key(grab: *mut ffi::weston_keyboard_grab,
			  _time: *const ffi::timespec,
			  key: u32,
			  state: u32) {
	let switcher = unsafe { &mut *container_of_mut!(grab, super::Switcher, grab) };
	let seat = unsafe { &mut *(*switcher.grab.keyboard).seat };

	let forwards = !seat.modifier_state.is_set(ffi::weston_keyboard_modifier::MODIFIER_SHIFT);

	if key == KEY_TAB &&
	    state == wl_keyboard_key_state::WL_KEYBOARD_KEY_STATE_PRESSED as u32 {
	    println!("tab");
		switcher.pick(forwards);
	    }
    }

    pub extern "C" fn modifiers(grab: *mut ffi::weston_keyboard_grab,
				_serial: u32,
				_mods_depressed: u32,
				_mods_latched: u32,
				_mods_locked: u32,
				_group: u32) {
	let switcher = unsafe { &mut *container_of_mut!(grab, super::Switcher, grab) };
	let seat = unsafe { &mut *(*switcher.grab.keyboard).seat };

	if !seat.modifier_state.is_set(ffi::weston_keyboard_modifier::MODIFIER_ALT) {
	    switcher.shutdown()
	}

	println!("modifier");
    }

    pub extern "C" fn cancel(grab: *mut ffi::weston_keyboard_grab) {
	let switcher = unsafe { &mut *container_of_mut!(grab, super::Switcher, grab) };

	switcher.shutdown();

	println!("cancel");
    }
}
    
extern "C"
fn view_destroy_notify(listener: *mut ffi::wl_listener,
		       _data: *mut ::std::os::raw::c_void) {
    let switcher = unsafe {
	&mut *container_of_mut!(listener, Switcher, view_destroy_listener)
    };

    switcher.current = None;
    switcher.pick(true);
}

extern "C"
fn binding(keyboard: *mut ffi::weston_keyboard,
           _time: *const ffi::timespec,
           _key: u32,
           data: *mut Void) {
    let switcher = unsafe { &mut *(data as *mut Switcher) };
    
    println!("alt-tab!");

    switcher.start(keyboard)
}

pub fn init(shell: &Rc<RefCell<Shell>>) {
    static GRAB_INTERFACE: ffi::weston_keyboard_grab_interface =
	ffi::weston_keyboard_grab_interface {
	    key: Some(grab::key),
	    modifiers: Some(grab::modifiers),
	    cancel: Some(grab::cancel)
	};

    let mut switcher = Box::new(Switcher {
	shell: Rc::clone(shell),
	grab: ffi::weston_keyboard_grab {
	    interface: &GRAB_INTERFACE as *const _,
	    keyboard: 0 as *mut _
	},
	current: None,
	view_destroy_listener: ffi::wl_listener::new(view_destroy_notify)
    });

    switcher.view_destroy_listener.link.init();

    let ptr = Box::into_raw(switcher);

    let shell = shell.borrow();
    let mut compositor = shell.compositor.borrow_mut();

    compositor.add_key_binding(KEY_TAB,
			       ffi::weston_keyboard_modifier::MODIFIER_ALT,
			       Some(binding), ptr as *mut Void);
    compositor.add_key_binding(KEY_TAB,
			       ffi::weston_keyboard_modifier::MODIFIER_ALT |
			       ffi::weston_keyboard_modifier::MODIFIER_SHIFT,
			       Some(binding), ptr as *mut Void);
}
