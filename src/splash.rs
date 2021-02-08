// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::ffi;

use crate::NULL_VOID;
use crate::Void;
use std::cell::RefCell;
use std::rc::Rc;

pub mod croscomp_protocol {
    #![allow(dead_code, non_camel_case_types, unused_unsafe, unused_variables)]
    #![allow(non_upper_case_globals, non_snake_case, unused_imports)]
    #![allow(missing_docs, clippy::all)]

    pub(crate) use wayland_sys as sys;
    pub(crate) use wayland_server::*;
    pub(crate) use wayland_commons::*;
    pub(crate) use wayland_commons::wire::*;
    pub(crate) use wayland_commons::map::*;

    use wayland_server::{
	protocol::{wl_compositor, wl_shell, wl_output, wl_surface},
	Display,
    };

    include!(concat!(env!("OUT_DIR"), "/croscomp_api.rs"));
}

struct Splash {
    shell: Rc<RefCell<crate::Shell>>,
    views: Vec<*mut ffi::weston_view>
}

impl Drop for Splash {
    fn drop(&mut self) {
	println!("drop splash");
    }
}

fn croscomp_splash_interface() -> *const ffi::wl_interface {
    <croscomp_protocol::croscomp_splash::CroscompSplash as wayland_server::Interface>
	::c_interface() as *const ffi::wl_interface
}

extern "C"
fn splash_committed(psurface: *mut ffi::weston_surface, _sx: i32, _sy: i32) {
    let surface = unsafe { &mut *psurface };
    let view = unsafe {
	&mut *container_of_mut!(surface.views.next, ffi::weston_view, surface_link)
    };
    let output = unsafe { &mut *surface.output };
    let compositor = unsafe { &mut *surface.compositor };
    let layer = &mut compositor.fade_layer;

    if !view.is_mapped {
	unsafe {
	    ffi::weston_layer_entry_insert(&mut layer.view_list, &mut view.layer_link);
	    ffi::weston_view_set_position(view, output.x as f32, output.y as f32);
	    surface.is_mapped = true;
	    view.is_mapped = true;
	    ffi::weston_compositor_schedule_repaint(compositor);
	}
    }
}

extern "C" fn dispatcher(
    _data: *const ::std::os::raw::c_void,
    object: *mut ::std::os::raw::c_void,
    opcode: u32,
    _message: *const ffi::wl_message,
    args: *mut ffi::wl_argument) -> ::std::os::raw::c_int {
    println!("dispatch!");

    use croscomp_protocol::croscomp_splash::Request as Request;

    let resource = object as *mut ffi::wl_resource;
    let client = unsafe { ffi::wl_resource_get_client(resource) };
    let splash = unsafe {
	&mut *(ffi::wl_resource_get_user_data(resource) as *mut Splash)
    };
    println!("client {:?}", client);

    let request = unsafe {
	<Request as wayland_server::MessageGroup>
	    ::from_raw_c(object, opcode, args as *const wayland_sys::common::wl_argument)
    };
    match request {
	Ok(Request::Set { output: output_resource, surface: surface_resource }) => {
	    let surface = unsafe {
		&mut *(ffi::wl_resource_get_user_data(
		    surface_resource.as_ref().c_ptr() as *mut ffi::wl_resource)
		   as *mut ffi::weston_surface)
	    };

	    let head = unsafe {
		&*(ffi::wl_resource_get_user_data(
		    output_resource.as_ref().c_ptr() as *mut ffi::wl_resource)
		    as *mut ffi::weston_head)
	    };

	    if surface.committed.is_some() {
		println!("surface already committed");
		return 0;
	    }

	    let view = unsafe {
		ffi::weston_view_create(surface)
	    };

	    surface.committed = Some(splash_committed);
	    surface.output = head.output;
	    unsafe {
		ffi::weston_view_set_output(view, head.output)
	    };

	    splash.views.push(view);
	    surface.ref_count += 1;
	},
	_ => { println!("invalid request") }
    };

    0
}

extern "C" fn unbind_splash(resource: *mut ffi::wl_resource) {
    let splash = unsafe {
	Box::from_raw(ffi::wl_resource_get_user_data(resource) as *mut Splash)
    };
    println!("unbind_splash");

    for &view in splash.views.iter() {
	unsafe { ffi::weston_surface_destroy((*view).surface) }
    }

    let shell = splash.shell.borrow();
    shell.finish_init()
}

extern "C" fn bind_splash(
    client: *mut ffi::wl_client,
    data: *mut ::std::os::raw::c_void,
    _version: u32,
    id: u32) {

    let r = unsafe {
	ffi::wl_resource_create(client, croscomp_splash_interface(), 1, id)
    };

    unsafe {
	ffi::wl_resource_set_dispatcher(r,
					Some(dispatcher), NULL_VOID,
					data, Some(unbind_splash))
    };
}

extern "C"
fn launch_splash_process(data: *mut ::std::os::raw::c_void) {
    
    let compositor = unsafe { &mut *(data as *mut ffi::weston_compositor) };
    let path = "/usr/bin/weston-simple-splash\0";

    unsafe {
	ffi::weston_client_start(compositor, path.as_ptr() as *const i8)
    };
}

pub fn init(shell: &Rc<RefCell<crate::Shell>>) {
    let s = Box::new(Splash {
	shell: Rc::clone(shell),
	views: Vec::new()
    });

    let p = Box::into_raw(s);

    let compositor = &shell.borrow().compositor;

    unsafe {
	ffi::wl_global_create(
	    (*compositor.borrow().native).wl_display,
	    croscomp_splash_interface(), 1,
	    p as *mut Void, Some(bind_splash));
    }

    let event_loop = unsafe {
	&mut *ffi::wl_display_get_event_loop((*compositor.borrow().native).wl_display)
    };

    unsafe {
	ffi::wl_event_loop_add_idle(event_loop,
				    Some(launch_splash_process),
				    compositor.borrow().native as *mut Void);
    }
}
