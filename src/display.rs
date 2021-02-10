// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cell::RefCell;
use std::rc::Rc;
use crate::ffi;
use crate::NULL_VOID;
use crate::Void;
use crate::protocol;

use wayland_server::MessageGroup;

struct Display {
    shell: Rc<RefCell<crate::Shell>>
}

fn croscomp_display_interface() -> *const ffi::wl_interface {
    <protocol::croscomp_display::CroscompDisplay as wayland_server::Interface>
	::c_interface() as *const ffi::wl_interface
}

fn croscomp_output_interface() -> *const ffi::wl_interface {
    <protocol::croscomp_output::CroscompOutput as wayland_server::Interface>
	::c_interface() as *const ffi::wl_interface
}

extern "C" fn output_dispatcher(
    _data: *const ::std::os::raw::c_void,
    _object: *mut ::std::os::raw::c_void,
    _opcode: u32,
    _message: *const ffi::wl_message,
    _args: *mut ffi::wl_argument) -> ::std::os::raw::c_int {
    0
}

extern "C" fn dispatcher(
    _data: *const ::std::os::raw::c_void,
    object: *mut ::std::os::raw::c_void,
    opcode: u32,
    _message: *const ffi::wl_message,
    args: *mut ffi::wl_argument) -> ::std::os::raw::c_int {
    println!("dispatch croscomp_display!");

    use protocol::croscomp_display::Request as Request;

    let request = unsafe {
	<Request as wayland_server::MessageGroup>
	    ::from_raw_c(object, opcode, args as *const wayland_sys::common::wl_argument)
    };

    match request {
	Ok(Request::Destroy) => {
	    println!("destroy");
	},
	_ => { panic!("illegal request") }
    }
    
    0
}

extern "C" fn unbind_display(_resource: *mut ffi::wl_resource) {
}

extern "C" fn unbind_output(_resource: *mut ffi::wl_resource) {
}

impl ffi::wl_resource {
    fn send<I>(&mut self, message: I)
    where I: MessageGroup {
	message.as_raw_c_in(
	    |opcode, args|
	    unsafe {
		ffi::wl_resource_post_event_array(
		    self, opcode,
		    args.as_mut_ptr() as *mut ffi::wl_argument);
	    });
    }
}

extern "C" fn bind_display(
    client: *mut ffi::wl_client,
    data: *mut ::std::os::raw::c_void,
    _version: u32,
    id: u32) {

    let display = unsafe { &mut *(data as *mut Display) };
    let version = 1;
    let raw_resource = unsafe {
	&mut *ffi::wl_resource_create(client, croscomp_display_interface(), version, id)
    };

    unsafe {
	ffi::wl_resource_set_dispatcher(
	    raw_resource,
	    Some(dispatcher), NULL_VOID,
	    (display as *mut Display) as *mut ::std::os::raw::c_void,
	    Some(unbind_display))
    };

    let compositor = &display.shell.borrow().compositor;

    for head in compositor.borrow_mut().head_iter().filter(|h| h.is_enabled()) {
	println!("head {:?}", head);

	let raw_output_resource = unsafe {
	    &mut *ffi::wl_resource_create(
		client, croscomp_output_interface(), version, 0)
	};

	unsafe {
	    ffi::wl_resource_set_dispatcher(
		raw_output_resource,
		Some(output_dispatcher), NULL_VOID,
		NULL_VOID,
		Some(unbind_output))
	};

	unsafe {
	    //use protocol::croscomp_display::Event as Event;

	    //raw_resource.send(Event::Output { id: output_resource })

	    let mut args = [ffi::wl_argument {
		o: raw_output_resource as *mut ffi::wl_resource
		    as *mut ffi::wl_object
	    }];
	    ffi::wl_resource_post_event_array(raw_resource, 0,
					      args.as_mut_ptr());
	};

	{ 
	    use protocol::croscomp_output::{Event, Subpixel, Transform};

	    let make = "panel".to_string();
	    let model = "foo".to_string();

	    raw_output_resource.send(Event::Geometry {
		physical_width: head.mm_width,
		physical_height: head.mm_height,
		subpixel: Subpixel::Unknown, // head.subpixel
		make: make,
		model: model,
		transform: Transform::Normal, // head.transform
	    });

	    if let Some(output) = unsafe { head.output.as_ref() } {
		println!("output {:?}", output);

		if output.enabled {
		    for mode in wl_list_iter!(output.mode_list,
					      ffi::weston_mode, link) {
			println!(" - mode {:?}", mode);

			raw_output_resource.send(Event::Mode {
			    flags: protocol::croscomp_output::Mode::Current |
			    protocol::croscomp_output::Mode::Preferred, // xxx
			    width: mode.width,
			    height: mode.height,
			    refresh: mode.refresh as i32
			});
		    }
		}
	    }

	    raw_output_resource.send(Event::Done {});
	}
    }
}

pub fn init(shell: &Rc<RefCell<crate::Shell>>) {
    let display = Box::new(Display {
	shell: Rc::clone(shell)
    });

    let compositor = &shell.borrow().compositor;

    let p = Box::into_raw(display);

    unsafe {
	ffi::wl_global_create(
	    (*compositor.borrow().native).wl_display,
	    croscomp_display_interface(), 1,
	    p as *mut Void, Some(bind_display));
    }
}
