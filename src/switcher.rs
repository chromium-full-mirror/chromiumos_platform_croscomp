// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cell::RefCell;
use std::rc::Rc;

use crate::Shell;
use crate::ffi;
use crate::ffi::evdev::KEY_TAB;
use crate::Void;

struct Thumbnail {
    view: *mut ffi::weston_view,
    transform: ffi::weston_transform
}

struct Switcher {
    shell: Rc<RefCell<crate::Shell>>,
    grab: ffi::weston_keyboard_grab,
    current: usize,
    view_destroy_listener: ffi::wl_listener,

    ui: Vec<*mut ffi::weston_view>,
    thumbnails: Vec<Thumbnail>
}

impl Switcher {
    fn start(&mut self, keyboard: *mut ffi::weston_keyboard) {
	let keyboard = unsafe { &mut *keyboard };

	unsafe {
	    ffi::lower_fullscreen_layer(&mut self.shell.borrow_mut().native, 0 as *mut _);
	}

	let output = unsafe { &(*(*keyboard.focus).output) };
	keyboard.start_grab(&mut self.grab as *mut _);
	keyboard.set_focus(0 as * mut _);

	self.build_ui(output);

	self.pick(true);
    }

    fn build_ui(&mut self, output: &ffi::weston_output) {
	let mut shell = self.shell.borrow_mut();
	let margin = 30;
	let q = output.height / 4;

	{
	    let compositor = shell.compositor.borrow_mut();

	    let mut surface = {
		unsafe { &mut (*compositor.create_surface()) }
	    };

	    surface.committed = None;
	    // xx set label func
	    surface.set_color(0.2, 0.2, 0.25, 0.99);
	    surface.set_size(output.width - 2 * margin, output.height - 2 * q);
	    surface.damage();

	    let view = unsafe { &mut *surface.create_view() };
	    view.set_position((output.x + margin) as f32, (output.y + q) as f32);
	    view.radius = 100.0;
	    view.geometry_dirty();
	    view.is_mapped = true;

	    self.ui.push(view);
	}

	{
	    let workspace = unsafe {
		&mut *ffi::get_current_workspace(&mut shell.native)
	    };

	    let mut x = 2 * margin;
	    self.thumbnails = wl_list_iter_mut!(
		&mut workspace.layer.view_list.link,
		ffi::weston_view, layer_link.link).map(
		|view| {
		    let surface = unsafe { &mut *view.surface };
		    let icon = unsafe { &mut *surface.create_view() };

		    // xxx need to use the shell surface extents here instead
		    let scale = (2 * q - 2 * margin) as f32 / surface.height as f32;
		    let y = (output.height - (surface.height as f32 * scale) as i32) / 2;
 		    icon.set_position(x as f32, y as f32);
		    icon.geometry_dirty();
		    icon.is_mapped = true;

		    x = x + (surface.width as f32 * scale) as i32 + margin;

		    let mut matrix = ffi::weston_matrix::new();
		    matrix.scale(scale, scale, 1.0);
		    Thumbnail {
			view: icon as *mut ffi::weston_view,
			transform: ffi::weston_transform {
			    matrix: matrix,
			    link: ffi::NULL_LIST
			}
		    }
		}).collect();

	    // Ugh, have to add the transform after adding to the Vec,
	    // so that the views have a stable memory location.
	    for t in &mut self.thumbnails {
		unsafe { ffi::wl_list_insert(
		    &mut (*t.view).geometry.transformation_list as *mut ffi::wl_list,
		    &mut t.transform.link as *mut ffi::wl_list);
		}
	    }
	}

	for view in self.ui.iter() {
	    unsafe {
		ffi::weston_layer_entry_insert(
		    &mut shell.native.fullscreen_layer.view_list,
		    &mut (**view).layer_link);
	    }
	}

	for t in self.thumbnails.iter() {
	    unsafe {
		ffi::weston_layer_entry_insert(
		    &mut shell.native.fullscreen_layer.view_list,
		    &mut (*t.view).layer_link);
	    }
	}
    }

    fn pick(&mut self, _forwards: bool) {
	let seat = unsafe { &mut *(*self.grab.keyboard).seat };

	let len = self.thumbnails.len();
	if seat.modifier_state.is_set(
	    ffi::weston_keyboard_modifier::MODIFIER_SHIFT) {
	    self.current = (self.current + len - 1) % len;
	} else {
	    self.current = (self.current + 1) % len;
	}

	for (i, t) in self.thumbnails.iter().enumerate() {
	    unsafe {
		let view = &mut *t.view;
		let surface = &mut *view.surface;
		view.alpha = if i == self.current { 1.0 } else { 0.25 };
		view.geometry_dirty();
		surface.damage();
	    }
	}
    }

    fn shutdown(&mut self) {
	let mut shell = self.shell.borrow_mut();
	let seat = unsafe { &mut *(*self.grab.keyboard).seat };

	unsafe { ffi::weston_keyboard_end_grab(self.grab.keyboard) };

	let t = &mut self.thumbnails[self.current as usize];
	unsafe {
	    ffi::activate(&mut shell.native, t.view, seat,
			  ffi::weston_activate_flag::WESTON_ACTIVATE_FLAG_CONFIGURE as u32);
	}

	for view in self.ui.drain(..) {
	    unsafe {
		let surface = (*view).surface;
		ffi::weston_surface_destroy(surface);
	    }
	}

	for t in self.thumbnails.drain(..) {
	    unsafe { ffi::weston_view_destroy(t.view) };
	}
    }
}

mod grab {
    use super::ffi;
    use ffi::evdev::KEY_TAB;
    use ffi::wl_keyboard_key_state;

    extern "C" fn key(grab: *mut ffi::weston_keyboard_grab,
		      _time: *const ffi::timespec,
		      key: u32,
		      state: u32) {
	let switcher = unsafe { &mut *container_of_mut!(grab, super::Switcher, grab) };
	let seat = unsafe { &mut *(*switcher.grab.keyboard).seat };

	let forwards = !seat.modifier_state.is_set(ffi::weston_keyboard_modifier::MODIFIER_SHIFT);

	if key == KEY_TAB &&
	    state == wl_keyboard_key_state::WL_KEYBOARD_KEY_STATE_PRESSED as u32 {
		switcher.pick(forwards);
	    }
    }

    extern "C" fn modifiers(grab: *mut ffi::weston_keyboard_grab,
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
    }

    extern "C" fn cancel(grab: *mut ffi::weston_keyboard_grab) {
	let switcher = unsafe { &mut *container_of_mut!(grab, super::Switcher, grab) };

	switcher.shutdown();
    }

    pub static INTERFACE: ffi::weston_keyboard_grab_interface =
	ffi::weston_keyboard_grab_interface {
	    key: Some(key),
	    modifiers: Some(modifiers),
	    cancel: Some(cancel)
	};
}
    
extern "C"
fn view_destroy_notify(listener: *mut ffi::wl_listener,
		       _data: *mut ::std::os::raw::c_void) {
    let switcher = unsafe {
	&mut *container_of_mut!(listener, Switcher, view_destroy_listener)
    };

    switcher.current = 0;
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
    let mut switcher = Box::new(Switcher {
	shell: Rc::clone(shell),
	grab: ffi::weston_keyboard_grab {
	    interface: &grab::INTERFACE as *const _,
	    keyboard: 0 as *mut _
	},
	current: 0,
	view_destroy_listener: ffi::wl_listener::new(view_destroy_notify),
	ui: Vec::new(),
	thumbnails: Vec::new()
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
