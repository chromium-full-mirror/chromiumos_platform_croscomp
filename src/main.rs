// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#[macro_use]
extern crate bitflags;

use std::env;
use std::fs::metadata;
use std::cell::RefCell;
use std::rc::Rc;

#[macro_use]
mod ffi;
mod terminal;
mod display;
mod splash;
mod switcher;
mod protocol;

const SCALE: i32 = 2;

fn verify_xdg_runtime_dir() {
    let path = env::var_os("XDG_RUNTIME_DIR")
	.expect("environment variable XDG_RUNTIME_DIR is not set.")
	.into_string()
	.expect("invalid value");

    let md = metadata(&path).unwrap();
    if !md.is_dir() {
	panic!("environment variable XDG_RUNTIME_DIR\nis set to \"{}\", \
		which is not a directory.", path);
    }

    // Verify permissions 0700 and owned by user?
}

const NULL_STR: *mut ::std::os::raw::c_char = 0 as *mut ::std::os::raw::c_char;
const NULL_VOID: *mut ::std::os::raw::c_void = 0 as *mut ::std::os::raw::c_void;
type Void = ::std::os::raw::c_void;

extern "C"
fn head_notify(_listener: *mut ffi::wl_listener, data: *mut ::std::os::raw::c_void) {
    let native = unsafe { &mut *(data as *mut ffi::weston_compositor) };
    let compositor = unsafe {
	&mut *container_of_mut!(native as *mut _, Compositor, native)
    };
    println!("listener");

    for head in compositor.head_iter_mut()
	.filter(|h| h.connected && !h.is_enabled()) {
	unsafe {
	    let api = ffi::weston_plugin_api_get(
		native,
		ffi::WESTON_DRM_OUTPUT_API_NAME.as_ptr() as *const i8,
		std::mem::size_of::<ffi::weston_drm_output_api>() as u64)
		as *mut ffi::weston_drm_output_api;

	    let output = native.create_output("output\0");
	    output.attach_head(head);

	    use ffi::weston_drm_backend_output_mode::*;
	    let set_mode = (*api).set_mode.unwrap();
	    set_mode(output, WESTON_DRM_BACKEND_OUTPUT_PREFERRED, NULL_STR);
	    output.set_scale(SCALE);

	    use ffi::wl_output_transform::*;
	    output.set_transform(WL_OUTPUT_TRANSFORM_NORMAL);
	    let set_gbm_format = (*api).set_gbm_format.unwrap();
	    set_gbm_format(output, NULL_STR);

	    let set_seat = (*api).set_seat.unwrap();
	    set_seat(output, "\0".as_ptr() as *const i8);

	    if ffi::weston_output_enable(output) != 0 {
		println!("weston_output_enable return non-0");
	    }

	    println!("attached to output {:?}", output);
	}
    }
}

#[derive(Debug)]
struct Display {
    native: *mut ffi::wl_display
}

impl Display {
    fn new() -> Self {
	Display { native: unsafe { ffi::wl_display_create() } }
    }

    fn run(&self) {
	unsafe { ffi::wl_display_run(self.native) }
    }

    fn add_socket_auto(&self) -> *const ::std::os::raw::c_char {
	unsafe { ffi::wl_display_add_socket_auto(self.native) }
    }
}

impl Drop for Display {
    fn drop(&mut self) {
	println!("drop display");
	unsafe { ffi::wl_display_destroy(self.native) }
    }
}

#[derive(Debug)]
pub struct Compositor {
    native: ffi::weston_compositor
}

impl Compositor {
    fn new() -> Self {
	Compositor { native: unsafe { std::mem::zeroed() } }
    }

    fn init(&mut self, display: &Display, ctx: *mut ffi::weston_log_context) {
	unsafe  {
	    ffi::weston_compositor_init(&mut self.native,
					display.native, ctx, NULL_VOID)
	};
    }

    fn add_heads_changed_listener(&mut self, listener: &mut ffi::wl_listener) {
	unsafe {
	    ffi::weston_compositor_add_heads_changed_listener(&mut self.native,
							      listener)
	}
    }

    fn add_key_binding(&mut self, key: u32,
		       modifier: ffi::weston_keyboard_modifier,
		       binding: ffi::weston_key_binding_handler_t,
		       data: *mut ::std::os::raw::c_void) -> *mut ffi::weston_binding{
	unsafe {
	    ffi::weston_compositor_add_key_binding(
		&mut self.native, key, modifier, binding, data)
	}
    }

    fn wake(&mut self) {
	unsafe { ffi::weston_compositor_wake(&mut self.native) }
    }

    fn flush_heads_changed(&mut self) {
	unsafe {
	    ffi::weston_compositor_flush_heads_changed(&mut self.native)
	}
    }

    fn set_xkb_rule_names(&mut self, names: &mut ffi::xkb_rule_names) {
	unsafe {
	    ffi::weston_compositor_set_xkb_rule_names(&mut self.native, names);
	}
    }

    fn head_iter(&self) -> ffi::WlListIter<ffi::weston_head> {
	wl_list_iter!(&self.native.head_list, ffi::weston_head, compositor_link)
    }

    fn head_iter_mut(&mut self) -> ffi::WlListIterMut<ffi::weston_head> {
	wl_list_iter_mut!(&mut self.native.head_list, ffi::weston_head, compositor_link)
    }

    fn create_surface(&self) -> *mut ffi::weston_surface {
	unsafe { ffi::weston_surface_create(&self.native
					    as *const _ as *mut _) }
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
	println!("drop compositor");
	unsafe { ffi::weston_compositor_deinit(&mut self.native) }
    }
}


#[derive(Debug)]
pub struct Shell {
    native: ffi::desktop_shell,
    compositor: Rc<RefCell<Compositor>>
}

impl Shell {
    fn new(compositor: &Rc<RefCell<Compositor>>) -> Self {
	Shell {
	    native: unsafe { std::mem::zeroed() },
	    compositor: Rc::clone(compositor)
	}
    }

    fn init(&mut self) {
	unsafe {
	    let native = &mut self.compositor.borrow_mut().native;
	    ffi::desktop_shell_init(&mut self.native, native)
	};

	self.create_background_surface();
    }

    fn finish_init(&mut self) {
	unsafe {
	    ffi::weston_compositor_init_input(&mut self.compositor.borrow_mut().native);
	    ffi::exo_extensions_init(&mut self.native);
	}
    }

    fn create_background_surface(&mut self) {
	let compositor = self.compositor.borrow_mut();
	let mut surface = unsafe { &mut (*compositor.create_surface()) };

	surface.committed = None;
	// xx set label func
	surface.set_size(1024 * 1024, 1024 * 1024);
	surface.set_color(0.2, 0.1, 0.1, 1.0);
	surface.damage();

	let view = unsafe { &mut *surface.create_view() };
	view.set_position(0.0, 0.0);
	view.geometry_dirty();
	view.is_mapped = true;

	unsafe {
	    ffi::weston_layer_entry_insert(&mut self.native.background_layer.view_list,
					   &mut view.layer_link);
	}

    }

}

fn main() {
    verify_xdg_runtime_dir();

    let ctx = unsafe {
	ffi::weston_log_ctx_create()
    };

    let display = Display::new();

    let compositor =
	Rc::new(RefCell::new(Compositor::new()));
    compositor.borrow_mut().init(&display, ctx);

    println!("set log handler");
    unsafe {
	ffi::weston_log_set_handler(Some(ffi::vlog),
				    Some(ffi::vlog_continue))
    }

    println!("done set log handler");

    let mut listener = ffi::wl_listener::new(head_notify);

    compositor.borrow_mut().add_heads_changed_listener(&mut listener);

    let mut rules = ffi::xkb_rule_names {
	rules: NULL_STR,
	model: NULL_STR,
	layout: NULL_STR,
	variant: NULL_STR,
	options: NULL_STR
    };

    compositor.borrow_mut().set_xkb_rule_names(&mut rules);

    let mut config = ffi::weston_drm_backend_config {
	base: ffi::weston_backend_config {
	    struct_version: ffi::WESTON_DRM_BACKEND_CONFIG_VERSION,
	    struct_size: std::mem::size_of::<ffi::weston_drm_backend_config>() as u64
	},
	tty: -1,
	use_pixman: false,
	seat_id: NULL_STR,
	gbm_format: NULL_STR,
	configure_device: None,
	pageflip_timeout: 0,
	specific_device: NULL_STR,
	use_pixman_shadow: false,
	continue_without_input: true
    };

    let _ret = unsafe { ffi::weston_compositor_load_backend(
	&mut compositor.borrow_mut().native,
	ffi::weston_compositor_backend::WESTON_BACKEND_DRM,
	&mut config.base)
    };

    compositor.borrow_mut().flush_heads_changed();

    let socket_name = display.add_socket_auto();

    if socket_name == NULL_STR {
	panic!("failed to create listening socket")
    }

    let shell = Rc::new(RefCell::new(Shell::new(&compositor)));
    shell.borrow_mut().init();

    display::init(&shell);

    terminal::init(&shell);

    splash::init(&shell);

    switcher::init(&shell);

    compositor.borrow_mut().wake();

    display.run();
}
