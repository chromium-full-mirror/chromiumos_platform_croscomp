// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cell::RefCell;
use std::rc::Rc;

mod protocol {
    #![allow(dead_code, non_camel_case_types, unused_unsafe, unused_variables)]
    #![allow(non_upper_case_globals, non_snake_case, unused_imports)]

    pub(crate) use wayland_server::*;
    pub(crate) use wayland_commons::*;
    pub(crate) use wayland_commons::wire::*;
    pub(crate) use wayland_commons::map::*;

    use wayland_server::{
	protocol::{wl_compositor, wl_shell, wl_output, wl_surface},
	Display,
    };

    include!(concat!(env!("OUT_DIR"), "/alpha-compositing-unstable-v1.rs"));
}

pub fn init(shell: &Rc<RefCell<crate::Shell>>) {
    let compositor = &shell.borrow().compositor;
    let d = unsafe { &mut (*compositor.borrow().native.wl_display) };

    let version = 1;
    let global = d.create_global::<protocol::zcr_alpha_compositing_v1::ZcrAlphaCompositingV1>(version)
	.on_request(|_resource, request| {
	    use protocol::zcr_alpha_compositing_v1::Request;
	    match request {
		Request::Destroy => {
		    println!("destroy");
		},

		Request::GetBlending { .. } => {
		},
	    }
	});

    Box::into_raw(Box::new(global));
}
	
