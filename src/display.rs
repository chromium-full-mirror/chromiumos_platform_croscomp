// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cell::RefCell;
use std::rc::Rc;
use crate::ffi;
use crate::protocol;

use super::protocol::croscomp_display::CroscompDisplay;
use super::protocol::croscomp_output::CroscompOutput;

extern "C" fn handle_bind(resource: &mut super::Resource<CroscompDisplay>,
			  compositor: &mut super::Compositor) {
    for head in compositor.head_iter().filter(|h| h.is_enabled()) {
	println!("head {:?}", head);

	let mut output_resource =
	    super::Resource::<CroscompOutput>::new(resource.client(), 1);

	unsafe {
	    //use protocol::croscomp_display::Event as Event;

	    //raw_resource.send(Event::Output { id: output_resource })

	    let mut args = [ffi::wl_argument {
		o: output_resource.native as *mut ffi::wl_object
	    }];
	    ffi::wl_resource_post_event_array(resource.native, 0,
					      args.as_mut_ptr());
	};

	{ 
	    use super::protocol::croscomp_output::{Event, Subpixel, Transform};

	    let make = "panel".to_string();
	    let model = "foo".to_string();

	    output_resource.send(Event::Geometry {
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

			output_resource.send(Event::Mode {
			    flags: protocol::croscomp_output::Mode::Current |
			    protocol::croscomp_output::Mode::Preferred, // xxx
			    width: mode.width,
			    height: mode.height,
			    refresh: mode.refresh as i32
			});
		    }
		}
	    }

	    output_resource.send(Event::Done {});
	}
    }
}

pub fn init(shell: &Rc<RefCell<crate::Shell>>) {
    let rc = Rc::clone(shell);
    let shell = &shell.borrow();
    let mut compositor = shell.compositor.borrow_mut();

    let global = compositor.create_global::<CroscompDisplay>(1)
	.on_bind(move |resource| {
	    let shell = &rc.borrow();
	    let mut compositor = shell.compositor.borrow_mut();

	    handle_bind(resource, &mut compositor)
	})
	.on_request(|_resource, request| {
	    println!("{:?}", request)
	});
    Box::into_raw(Box::new(global));

}
