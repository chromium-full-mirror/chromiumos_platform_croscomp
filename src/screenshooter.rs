// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::ffi;
use crate::protocol;
use crate::Shell;

use log::debug;
use std::cell::RefCell;
use std::rc::Rc;

use crate::server::Resource;
use protocol::weston_screenshooter::{Request, WestonScreenshooter};

struct ScreenShooter {
    shell: Rc<RefCell<crate::Shell>>,
    client: Option<*mut ffi::wl_client>,
}

impl ScreenShooter {
    fn shoot(&mut self) {
        let path = "/usr/bin/weston-screenshooter";
        let shell = self.shell.borrow();
        let mut compositor = shell.compositor.borrow_mut();

        if self.client.is_none() {
            self.client = compositor.launch_client(path).ok();
        }
    }

    fn record(&mut self) {
        debug!("screenshooter record");
    }
}

fn handle_request(resource: &mut Resource<WestonScreenshooter>, request: &Request) {
    unsafe extern "C" fn done(
        data: *mut ::std::ffi::c_void,
        outcome: ffi::weston_screenshooter_outcome,
    ) {
        use protocol::weston_screenshooter::Event::Done;
        let resource = Resource::<WestonScreenshooter>::from_c_ptr(data as *mut _);

        match outcome {
            ffi::weston_screenshooter_outcome::WESTON_SCREENSHOOTER_SUCCESS => resource.send(Done),
            _ => resource.post_no_memory(),
        }
    }

    match request {
        Request::Shoot { output, buffer } => unsafe {
            let output = (*ffi::weston_head_from_resource(output.as_ref().c_ptr())).output;
            let buffer = ffi::weston_buffer_from_resource(buffer.as_ref().c_ptr());

            ffi::weston_screenshooter_shoot(output, buffer, Some(done), resource.c_ptr() as *mut _);
        },
    }
}

pub fn init(shell: &Rc<RefCell<Shell>>) {
    let shooter = Rc::new(RefCell::new(ScreenShooter {
        shell: Rc::clone(shell),
        client: None,
    }));

    let s = shell.borrow();
    let mut compositor = s.compositor.borrow_mut();
    let rc = Rc::clone(&shooter);
    let destroy_rc = Rc::clone(&shooter);

    let version = 1;
    let global = compositor
        .server
        .create_global::<WestonScreenshooter>(version)
        .on_bind(move |resource| {
            match rc.borrow().client {
                Some(native_client) if native_client == resource.client() => {
                    // good case
                    true
                }
                _ => {
                    resource.post_error(
                        ffi::wl_display_error::WL_DISPLAY_ERROR_INVALID_OBJECT as u32,
                        "screenshooter failed: permission denied",
                    );
                    false
                }
            }
        })
        .on_request(|resource, request| handle_request(resource, request))
        .on_destroy(move |_resource| {
            let mut s = destroy_rc.borrow_mut();
            s.client = None;
        });

    Box::into_raw(Box::new(global));

    use crate::ffi::evdev::{KEY_R, KEY_S};

    let rc = Rc::clone(&shooter);
    compositor.bind_key(
        KEY_S,
        ffi::weston_keyboard_modifier::MODIFIER_SUPER,
        move |_, _, _| rc.borrow_mut().shoot(),
    );

    let rc = Rc::clone(&shooter);
    compositor.bind_key(
        KEY_R,
        ffi::weston_keyboard_modifier::MODIFIER_SUPER,
        move |_keyboard, _, _| rc.borrow_mut().record(),
    );
}
