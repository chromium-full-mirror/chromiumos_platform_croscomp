// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::ffi;
use crate::protocol;
use log::{debug, error, warn};
use std::cell::RefCell;
use std::rc::Rc;

use crate::server::Resource;
use protocol::xdg_popup::{self, XdgPopup};
use protocol::xdg_positioner::{self, XdgPositioner};
use protocol::xdg_surface::{self, XdgSurface};
use protocol::xdg_toplevel::{self, XdgToplevel};
use protocol::xdg_wm_base::{self, XdgWmBase};

struct Positioner {
    size: ffi::weston_size,
    anchor_rect: ffi::weston_geometry,
    anchor: xdg_positioner::Anchor,
    offset: ffi::weston_position,
    gravity: xdg_positioner::Gravity,
    constraint_adjustment: xdg_positioner::ConstraintAdjustment,
}

impl Positioner {
    fn new() -> Positioner {
        Positioner {
            size: ffi::weston_size { width: 0, height: 0 },
            anchor_rect: ffi::weston_geometry { x: 0, y: 0, width: 0, height: 0 },
            anchor: xdg_positioner::Anchor::None,
            offset: ffi::weston_position { x: 0, y: 0 },
            gravity: xdg_positioner::Gravity::None,
            constraint_adjustment: xdg_positioner::ConstraintAdjustment::None,
        }
    }
}

fn handle_positioner_request(
    _resource: &mut Resource<XdgPositioner>,
    request: &xdg_positioner::Request,
) {
    let mut p = Positioner::new();

    debug!("{:?}", request);

    use xdg_positioner::Request::*;
    match *request {
        Destroy => {}
        SetSize { width, height } => {
            if width < 1 || height < 1 {
                error!("width and height must me non-negative");
                // xxx send error and return
            }
            p.size = ffi::weston_size { width, height };
        }
        SetAnchorRect { x, y, width, height } => {
            if width < 0 || height < 0 {
                error!("width and height must me non-negative");
                // xxx send error and return
            }
            p.anchor_rect = ffi::weston_geometry { x, y, width, height };
        }
        SetAnchor { anchor } => p.anchor = anchor,
        SetGravity { gravity } => p.gravity = gravity,
        SetConstraintAdjustment { constraint_adjustment } => {
            match xdg_positioner::ConstraintAdjustment::from_raw(constraint_adjustment) {
                Some(adjustment) => p.constraint_adjustment = adjustment,
                _ => error!("send error"),
            }
        }
        SetOffset { x, y } => p.offset = ffi::weston_position { x, y },
        SetReactive => {}
        SetParentSize { parent_width: _, parent_height: _ } => {}
        SetParentConfigure { serial: _ } => {}
    }
}

fn handle_xdg_toplevel_request(
    _resource: &mut Resource<XdgToplevel>,
    request: &xdg_toplevel::Request,
) {
    use xdg_toplevel::Request::*;

    match request {
        Destroy => (),
        SetParent { parent: _ } => (),
        SetTitle { title: _ } => (),
        SetAppId { app_id: _ } => (),
        ShowWindowMenu {
            seat: _,
            serial: _,
            x: _,
            y: _,
        } => (),
        Move { seat: _, serial: _ } => (),
        Resize {
            seat: _,
            serial: _,
            edges: _,
        } => (),
        SetMaxSize {
            width: _,
            height: _,
        } => (),
        SetMinSize {
            width: _,
            height: _,
        } => (),
        SetMaximized => (),
        UnsetMaximized => (),
        SetFullscreen { output: _ } => (),
        UnsetFullscreen => (),
        SetMinimized => (),
    }
}

fn handle_xdg_popup_request(_resource: &mut Resource<XdgPopup>, request: &xdg_popup::Request) {
    use xdg_popup::Request::*;

    match request {
        Destroy => (),
        Grab { seat: _, serial: _ } => (),
        Reposition {
            positioner: _,
            token: _,
        } => (),
    }
}

fn handle_xdg_surface_request(resource: &mut Resource<XdgSurface>, request: &xdg_surface::Request) {
    use xdg_surface::Request::*;
    use xdg_wm_base::Error;

    match request {
        Destroy => (),
        GetToplevel { id } => {
            id.on_request(handle_xdg_toplevel_request);
        }
        GetPopup {
            id,
            parent,
            positioner: _,
        } => {
            if parent.is_none() {
                resource.post_error(
                    Error::InvalidPopupParent as u32,
                    "popup parent must be non-null",
                );
                return;
            };
            id.on_request(handle_xdg_popup_request);
        }
        SetWindowGeometry {
            x: _,
            y: _,
            width: _,
            height: _,
        } => (),
        AckConfigure { serial: _ } => (),
    }
}

fn handle_xdg_wm_base_request(_resource: &mut Resource<XdgWmBase>, request: &xdg_wm_base::Request) {
    use xdg_wm_base::Request::*;

    match request {
        Destroy => {
            // drop resource
            debug!("destroy")
        }
        CreatePositioner { id } => {
            debug!("create positioner");
            id.on_request(handle_positioner_request);
        }
        GetXdgSurface { id, surface: _ } => {
            debug!("get xdg surface");
            id.on_request(handle_xdg_surface_request);
        }
        Pong { serial: _ } => {
            // update timer
            // end busy cursor
            debug!("pong");
        }
    }
}

pub fn init(rc: &Rc<RefCell<crate::Shell>>) {
    let shell = &rc.borrow();
    let mut compositor = shell.compositor.borrow_mut();

    let global = compositor
        .server
        .create_global::<XdgWmBase>(1)
        .on_bind(|_| {
            warn!("xdg_wm_base: handle bind");
            /* set up ping timer */
            true
        })
        .on_request(handle_xdg_wm_base_request)
        .on_destroy(|_resource| debug!("destroy"));

    Box::into_raw(Box::new(global));
}
