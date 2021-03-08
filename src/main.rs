// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#[macro_use]
extern crate bitflags;

use std::env;
use std::fs::metadata;
use std::cell::RefCell;
use std::rc::Rc;
use std::marker::PhantomData;

#[macro_use]
mod ffi;
mod terminal;
mod display;
mod splash;
mod switcher;
mod move_resize;
mod protocol;
mod input;
mod rotate;
mod exo;

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
	    set_mode(output, WESTON_DRM_BACKEND_OUTPUT_PREFERRED, std::ptr::null());
	    output.set_scale(SCALE);

	    use ffi::wl_output_transform::*;
	    output.set_transform(WL_OUTPUT_TRANSFORM_NORMAL);
	    let set_gbm_format = (*api).set_gbm_format.unwrap();
	    set_gbm_format(output, std::ptr::null());

	    let set_seat = (*api).set_seat.unwrap();
	    set_seat(output, "\0".as_ptr() as *const i8);

	    if ffi::weston_output_enable(output) != 0 {
		println!("weston_output_enable return non-0");
	    }
	}
    }
}

pub struct Resource<I> {
    native: *mut ffi::wl_resource,
    phantom: PhantomData<I>
}

struct GlobalHandlers<I>
where I: wayland_server::Interface {
    handle_bind: Option<Box<dyn Fn(&mut Resource<I>)>>,
    dispatcher: Option<Box<dyn Fn(&mut Resource<I>, &I::Request)>>,
    handle_destroy: Option<Box<dyn Fn(&mut Resource<I>)>>,
}

pub struct Global<I>
where I: wayland_server::Interface {
    native: *mut ffi::wl_global,
    handlers: Box<GlobalHandlers<I>>
}

impl<I: wayland_server::Interface> Global<I> {
    pub fn on_bind<F>(mut self, f: F) -> Self
    where F: 'static + Fn(&mut Resource<I>) {
	// hook for handling bind time checks or allocations
	self.handlers.handle_bind = Some(Box::new(f));
	self
    }

    pub fn on_request<F>(mut self, f: F) -> Self
    where F: 'static + Fn(&mut Resource<I>, &I::Request) {
	println!("set dispatcher for {}: {:?}", I::NAME, &f as *const _);
	self.handlers.dispatcher = Some(Box::new(f));
	self
    }

    pub fn on_destroy<F>(mut self, f: F) -> Self
    where F: 'static + Fn(&mut Resource<I>) {
	self.handlers.handle_destroy = Some(Box::new(f));
	self
    }
}

impl<I: wayland_server::Interface> Resource<I> {
    pub fn set_dispatcher<F>(&mut self,
			     f: F)
    where F: 'static + Fn(&mut Resource<I>, &I::Request) {
	extern "C" fn dispatcher<I, F>(
	    data: *const ::std::ffi::c_void,
	    object: *mut ::std::ffi::c_void,
	    opcode: u32,
	    _message: *const ffi::wl_message,
	    args: *mut ffi::wl_argument) -> ::std::os::raw::c_int
	where I: wayland_server::Interface,
	      F: 'static + Fn(&mut Resource<I>, &I::Request) {
	    let request = unsafe {
		<I::Request as wayland_server::MessageGroup>
		    ::from_raw_c(object, opcode, args as *const wayland_sys::common::wl_argument)
	    };

	    let mut resource = Resource::<I> {
		native: object as *mut ffi::wl_resource,
		phantom: PhantomData
	    };
	    let handler = unsafe { &*(data as *mut Box<F>) };
	    if let Ok(request) = request {
		(*handler)(&mut resource, &request);
	    } else {
		panic!("illegal request");
	    };

	    0
	}

	let b = Box::new(f);
	unsafe {
	    ffi::wl_resource_set_dispatcher(
		self.native,
		Some(dispatcher::<I, F>),
		Box::into_raw(b) as *mut _,
		std::ptr::null_mut(),
		None)
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
    native: ffi::weston_compositor,
}

impl Compositor {
    fn new() -> Self {
	Compositor { native: unsafe { std::mem::zeroed() }, }
    }

    fn init(&mut self, display: &Display, ctx: *mut ffi::weston_log_context) {
	unsafe  {
	    ffi::weston_compositor_init(&mut self.native,
					display.native, ctx, std::ptr::null_mut())
	};
    }

    fn add_heads_changed_listener(&mut self, listener: &mut ffi::wl_listener) {
	unsafe {
	    ffi::weston_compositor_add_heads_changed_listener(&mut self.native,
							      listener)
	}
    }

    fn bind_key<F>(&mut self, key: u32,
		   modifier: ffi::weston_keyboard_modifier,
		   f: F)
    where F: Fn(&mut ffi::weston_keyboard, *const ffi::timespec, u32) {
	extern "C"
	fn call_key_closure<F>(keyboard: *mut ffi::weston_keyboard,
			       time: *const ffi::timespec,
			       key: u32,
			       data: *mut std::ffi::c_void)
	where F: Fn(&mut ffi::weston_keyboard, *const ffi::timespec, u32) {
	    let keyboard = unsafe { &mut *keyboard };
	    (unsafe { &mut *(data as *mut F) })(keyboard, time, key)
	}

	let handler = Box::new(f);

	unsafe {
	    ffi::weston_compositor_add_key_binding(
		&mut self.native, key, modifier,
		Some(call_key_closure::<F>),
		Box::into_raw(handler) as *mut _)
	};
    }

    fn bind_button<F>(&mut self, button: u32,
		      modifier: ffi::weston_keyboard_modifier,
		      f: F)
    where F: Fn(&mut ffi::weston_pointer, *const ffi::timespec, u32) {
	extern "C"
	fn call_button_closure<F>(pointer: *mut ffi::weston_pointer,
				  time: *const ffi::timespec,
				  key: u32,
				  data: *mut std::ffi::c_void)
	where F: Fn(&mut ffi::weston_pointer, *const ffi::timespec, u32) {
	    let pointer = unsafe { &mut *pointer };
	    (unsafe { &mut *(data as *mut F) })(pointer, time, key)
	}

	let handler = Box::new(f);

	unsafe {
	    ffi::weston_compositor_add_button_binding(
		&mut self.native, button, modifier,
		Some(call_button_closure::<F>),
		Box::into_raw(handler) as *mut _)
	};
    }

    fn when_idle<F>(&mut self, f: F)
    where F: FnOnce() {
	let event_loop = unsafe {
	    &mut *ffi::wl_display_get_event_loop(self.native.wl_display)
	};

	extern "C" fn call_closure<F>(data: *mut ::std::os::raw::c_void)
	where F: FnOnce() {
	    let f = unsafe { Box::from_raw(data as *mut F) };

	    f()
	}

	let p = Box::into_raw(Box::new(f));

	unsafe {
	    ffi::wl_event_loop_add_idle(event_loop,
					Some(call_closure::<F>), p as *mut _);
	}
    }

    fn start_client(&mut self, path: &str) {
	unsafe {
	    ffi::weston_client_start(&mut self.native, path.as_ptr() as *const i8)
	};
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

    fn finish_init(rc: &Rc<RefCell<crate::Shell>>) {
	unsafe {
	    let shell = &rc.borrow();
	    ffi::weston_compositor_init_input(&mut shell.compositor.borrow_mut().native);
	}
	exo::init(rc);
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
	rules: std::ptr::null(),
	model: std::ptr::null(),
	layout: std::ptr::null(),
	variant: std::ptr::null(),
	options: std::ptr::null()
    };

    compositor.borrow_mut().set_xkb_rule_names(&mut rules);

    let mut config = ffi::weston_drm_backend_config {
	base: ffi::weston_backend_config {
	    struct_version: ffi::WESTON_DRM_BACKEND_CONFIG_VERSION,
	    struct_size: std::mem::size_of::<ffi::weston_drm_backend_config>() as u64
	},
	tty: -1,
	use_pixman: false,
	seat_id: std::ptr::null_mut(),
	gbm_format: std::ptr::null_mut(),
	configure_device: None,
	pageflip_timeout: 0,
	specific_device: std::ptr::null_mut(),
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

    if socket_name == std::ptr::null() {
	panic!("failed to create listening socket")
    }

    let shell = Rc::new(RefCell::new(Shell::new(&compositor)));
    shell.borrow_mut().init();

    display::init(&shell);

    terminal::init(&shell);

    splash::init(&shell);

    switcher::init(&shell);

    move_resize::init(&shell);

    rotate::init(&shell);

    compositor.borrow_mut().wake();

    display.run();
}
