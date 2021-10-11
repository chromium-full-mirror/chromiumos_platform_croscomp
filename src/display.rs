// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use log::debug;

use crate::ffi;
use crate::protocol;
use std::cell::RefCell;
use std::rc::Rc;

use super::protocol::croscomp_display::CroscompDisplay;
use super::protocol::croscomp_output::CroscompOutput;
use crate::server::Resource;

fn handle_bind(
    resource: &mut Resource<CroscompDisplay>,
    compositor: &mut super::Compositor,
) -> bool {
    for head in compositor.head_iter().filter(|h| h.is_enabled()) {
        use super::protocol::croscomp_display::Event::Output;
        use super::protocol::croscomp_output::Event::{Done, Geometry, Mode};
        use super::protocol::croscomp_output::{Subpixel, Transform};

        let output_resource = resource.new::<CroscompOutput>(0);
        resource.send(Output {
            id: output_resource.clone(),
        });

        let make = "panel".to_string();
        let model = "foo".to_string();

        output_resource.send(Geometry {
            physical_width: head.mm_width,
            physical_height: head.mm_height,
            subpixel: Subpixel::Unknown, // head.subpixel
            make: make,
            model: model,
            transform: Transform::Normal, // head.transform
        });

        if let Some(output) = unsafe { head.output.as_ref() } {
            if output.enabled {
                for mode in wl_list_iter!(output.mode_list, ffi::weston_mode, link) {
                    output_resource.send(Mode {
                        flags: protocol::croscomp_output::Mode::Current
                            | protocol::croscomp_output::Mode::Preferred, // xxx
                        width: mode.width,
                        height: mode.height,
                        refresh: mode.refresh as i32,
                    });
                }
            }
        }

        output_resource.send(Done {});
    }

    true
}

pub fn init(shell: &Rc<RefCell<crate::Shell>>) {
    let rc = Rc::clone(shell);
    let shell = &shell.borrow();
    let mut compositor = shell.compositor.borrow_mut();

    let global = compositor
        .server
        .create_global::<CroscompDisplay>(1)
        .on_bind(move |resource| {
            let shell = &rc.borrow();
            let mut compositor = shell.compositor.borrow_mut();

            handle_bind(resource, &mut compositor)
        })
        .on_request(|_resource, request| debug!("{:?}", request));
    Box::into_raw(Box::new(global));
}
