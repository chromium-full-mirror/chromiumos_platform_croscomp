// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cell::RefCell;
use std::rc::Rc;

use super::protocol::zcr_remote_shell_v1::ZcrRemoteShellV1;

pub fn init(rc: &Rc<RefCell<crate::Shell>>) {
    let shell = &rc.borrow();
    let mut compositor = shell.compositor.borrow_mut();

    let rc = Rc::clone(rc);
    let global = compositor
        .server
        .create_global::<ZcrRemoteShellV1>(30)
        .on_bind(move |resource| {
            use super::protocol::wl_output;
            use super::protocol::zcr_remote_shell_v1::Event::*;
            use super::protocol::zcr_remote_shell_v1::LayoutMode;
            use super::protocol::zcr_remote_surface_v1::SystemuiVisibilityState;;

            let shell = &rc.borrow();
            let mut compositor = shell.compositor.borrow_mut();

            println!("on_bind, send workspace info");

            for head in compositor.head_iter_mut().filter(|h| !h.output.is_null()) {
                let output = unsafe { &*head.output };
                resource.send(WorkspaceInfo {
                    display_id_hi: 0,
                    display_id_lo: 0,
                    x: output.x,
                    y: output.y,
                    width: output.width,
                    height: output.height,
                    inset_left: 0,
                    inset_top: 0,
                    inset_right: 0,
                    inset_bottom: 0,
                    stable_inset_left: 0,
                    stable_inset_top: 0,
                    stable_inset_right: 0,
                    stable_inset_bottom: 0,
                    systemui_visibility: SystemuiVisibilityState::Visible.to_raw() as i32,
                    transform: wl_output::Transform::Normal.to_raw() as i32,
                    is_internal: head.connection_internal as u32,
                    identification_data: Vec::new(),
                });
            }

            // xxx create desktop client

            resource.send(DefaultDeviceScaleFactor { scale: 2 << 24 });
            resource.send(Configure {
                layout_mode: LayoutMode::Windowed.to_raw(),
            });
        })
        .on_request(|_resource, request| {
            use super::protocol::zcr_remote_shell_v1::Request::*;
            match request {
                Destroy => {}
                GetRemoteSurface {
                    id,
                    surface: _,
                    container: _,
                } => {
                    id.on_request(|_resource, request| match request {
                        _ => println!("request: {:?}", request),
                    });
                }
                GetNotificationSurface {
                    id: _,
                    surface: _,
                    notification_key: _,
                } => {}
                GetInputMethodSurface { id: _, surface: _ } => {}
                GetToastSurface { id: _, surface: _ } => {}
                GetRemoteOutput { id: _, output: _ } => {}
                SetUseDefaultDeviceScaleCancellation {
                    use_default_device_scale_factor: _,
                } => {}
            }
        });
    Box::into_raw(Box::new(global));
}
