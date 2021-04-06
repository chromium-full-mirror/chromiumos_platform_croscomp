// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::ffi;
use crate::protocol;
use std::cell::RefCell;
use std::rc::Rc;

use crate::server::Resource;
use protocol::croscomp_splash::{CroscompSplash, Request};

extern "C" fn splash_committed(psurface: *mut ffi::weston_surface, _sx: i32, _sy: i32) {
    let surface = unsafe { &mut *psurface };
    let view =
        unsafe { &mut *container_of_mut!(surface.views.next, ffi::weston_view, surface_link) };
    let output = unsafe { &mut *surface.output };
    let compositor = unsafe { &mut *surface.compositor };
    let layer = &mut compositor.fade_layer;

    if !view.is_mapped {
        unsafe {
            ffi::weston_layer_entry_insert(&mut layer.view_list, &mut view.layer_link);
        }

        view.set_position(output.x as f32, output.y as f32);
        surface.is_mapped = true;
        view.is_mapped = true;
        compositor.schedule_repaint();
    }
}

fn handle_request(_resource: &mut Resource<CroscompSplash>, request: &Request) {
    match request {
        Request::Set {
            output: output_resource,
            surface: surface_resource,
        } => {
            let surface = surface_resource.as_ref().user_data::<ffi::weston_surface>();
            let head = output_resource.as_ref().user_data::<ffi::weston_head>();

            if surface.committed.is_some() {
                println!("surface already committed");
                return;
            }

            surface.committed = Some(splash_committed);
            surface.output = head.output;

            let view = unsafe { &mut *surface.create_view() };
            view.set_output(head.output);

            //surface.ref_count += 1;
        }
    };
}

pub fn init(rc: &Rc<RefCell<crate::Shell>>) {
    let shell = &rc.borrow();
    let mut compositor = shell.compositor.borrow_mut();

    let version = 1;
    let rc = Rc::clone(rc);

    println!("from splash, allocating global");
    let global = compositor
        .server
        .create_global::<CroscompSplash>(version)
        .on_bind(|_| println!("splash: handle bind"))
        .on_request(|resource, request| handle_request(resource, request))
        .on_destroy(move |_resource| super::Shell::finish_init(&rc));

    Box::into_raw(Box::new(global));

    let rc = Rc::clone(&shell.compositor);
    compositor.server.when_idle(move || {
        let path = "/usr/bin/weston-simple-splash\0";
        let mut compositor = rc.borrow_mut();
        compositor.start_client(path);
    });
}
