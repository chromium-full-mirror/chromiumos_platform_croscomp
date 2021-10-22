// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::ffi;
use crate::protocol;
use log::{debug, error};
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

    fn get_geometry(&self) -> ffi::weston_geometry {
        use xdg_positioner::{Anchor, Gravity};

        let anchor = ffi::weston_position {
            x: match self.anchor {
                Anchor::Left | Anchor::TopLeft | Anchor::BottomLeft => self.anchor_rect.x,
                Anchor::Right | Anchor::TopRight | Anchor::BottomRight => {
                    self.anchor_rect.x + self.anchor_rect.width
                }
                Anchor::None | Anchor::Top | Anchor::Bottom => {
                    self.anchor_rect.x + self.anchor_rect.width / 2
                }
            },
            y: match self.anchor {
                Anchor::Top | Anchor::TopLeft | Anchor::TopRight => self.anchor_rect.y,
                Anchor::Bottom | Anchor::BottomLeft | Anchor::BottomRight => {
                    self.anchor_rect.x + self.anchor_rect.height
                }
                Anchor::None | Anchor::Left | Anchor::Right => {
                    self.anchor_rect.y + self.anchor_rect.height / 2
                }
            },
        };

        let gravity = ffi::weston_position {
            x: match self.gravity {
                Gravity::Left | Gravity::TopLeft | Gravity::BottomLeft => self.size.width,
                Gravity::None | Gravity::Top | Gravity::Bottom => self.size.width / 2,
                Gravity::Right | Gravity::TopRight | Gravity::BottomRight => 0,
            },
            y: match self.gravity {
                Gravity::Top | Gravity::TopLeft | Gravity::TopRight => self.size.height,
                Gravity::None | Gravity::Left | Gravity::Right => self.size.height / 2,
                Gravity::Bottom | Gravity::BottomLeft | Gravity::BottomRight => 0,
            },
        };

        ffi::weston_geometry {
            x: self.offset.x + anchor.x - gravity.x,
            y: self.offset.y + anchor.y - gravity.y,
            width: self.size.width,
            height: self.size.height,
        }
    }
}

fn handle_positioner_request(
    resource: &mut Resource<XdgPositioner>,
    request: &xdg_positioner::Request,
) {
    let mut p = resource.user_data::<Positioner>();

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

#[derive(Default)]
struct State {
    maximized: bool,
    fullscreen: bool,
    resizing: bool,
    activated: bool,
}

struct Surface {
    shell: Rc<RefCell<crate::Shell>>,
    surface: *mut ffi::weston_surface,
    shsurf: *mut ffi::shell_surface,
    title: String,
    app_id: String,

    next_max_size: ffi::weston_size,
    next_min_size: ffi::weston_size,
    current_state: State,
    next_state: State,
    next_size: ffi::weston_size,
    pending_state: State,
    pending_size: ffi::weston_size,

    next_geometry: Option<ffi::weston_geometry>,
    configured: bool,
}

impl Surface {
    fn new(shell: Rc<RefCell<crate::Shell>>, surface: &mut ffi::weston_surface) -> Self {
        Surface {
            shell: shell,
            surface: surface as *mut _,
            shsurf: 0 as *mut _,
            title: String::new(),
            app_id: String::new(),
            next_max_size: ffi::weston_size { width: 0, height: 0 },
            next_min_size: ffi::weston_size { width: 0, height: 0 },
            current_state: State::default(),
            next_state: State::default(),
            next_size: ffi::weston_size { width: 0, height: 0 },
            pending_state: State::default(),
            pending_size: ffi::weston_size { width: 0, height: 0 },

            next_geometry: None,
            configured: false,
        }
    }

    fn schedule_configure(&mut self) {}

    fn committed(&mut self) {
        // to be called from shell.rs
    }

    fn close(&mut self) {
        // will be called from shell.rs
    }

    fn destroy(&mut self) {
        // called from shell.rs
    }
}

fn handle_xdg_toplevel_request(
    resource: &mut Resource<XdgToplevel>,
    request: &xdg_toplevel::Request,
) {
    use xdg_toplevel::Request::*;

    let surface = resource.user_data::<Surface>();
    let shsurf = unsafe { &mut *surface.shsurf };

    match request {
        Destroy => (),
        SetParent { parent: _ } => (),
        SetTitle { title } => surface.title = title.clone(),
        SetAppId { app_id } => surface.app_id = app_id.clone(),
        ShowWindowMenu { seat: _, serial: _, x: _, y: _ } => (),
        Move { seat, serial } => {
            let seat = seat.as_ref().user_data::<ffi::weston_seat>();
            if let Some(pointer) = seat.get_pointer() {
                if !pointer.focus.is_null()
                    && pointer.button_count > 0
                    && pointer.grab_serial == *serial
                {
                    //let focus = &mut *pointer.focus;
                    //let focus = ffi::weston_surface_get_main_surface(focus.surface);
                    // if (focus == surface
                    shsurf.r#move(pointer, true);
                }
            }
            if let Some(touch) = seat.get_touch() {
                if !touch.focus.is_null() && touch.grab_serial == *serial {}
            }
        }
        Resize { seat, serial, edges } => {
            let seat = seat.as_ref().user_data::<ffi::weston_seat>();
            if let Some(pointer) = seat.get_pointer() {
                if !pointer.focus.is_null()
                    && pointer.button_count > 0
                    && pointer.grab_serial == *serial
                {
                    // let focus = get main surf
                    shsurf.resize(pointer, edges.to_raw())
                }
            }
        }
        SetMaxSize { width, height } => {
            surface.next_max_size = ffi::weston_size { width: *width, height: *height }
        }
        SetMinSize { width, height } => {
            surface.next_min_size = ffi::weston_size { width: *width, height: *height }
        }
        SetMaximized => unsafe {
            surface.pending_state.maximized = true;
            surface.pending_size = ffi::shell_surface_set_maximized(shsurf, true);
            surface.schedule_configure();
        },
        UnsetMaximized => unsafe {
            surface.pending_state.maximized = false;
            surface.pending_size = ffi::shell_surface_set_maximized(shsurf, false);
            surface.schedule_configure();
        },
        SetFullscreen { output } => unsafe {
            let output = match output {
                Some(output) => output.as_ref().user_data::<ffi::weston_output>(),
                None => 0 as *mut _,
            };
            surface.pending_size = ffi::shell_surface_set_fullscreen(shsurf, true, output);
            surface.pending_state.fullscreen = true;
            surface.schedule_configure();
        },
        UnsetFullscreen => unsafe {
            surface.pending_size = ffi::shell_surface_set_fullscreen(shsurf, false, 0 as *mut _);
            surface.pending_state.fullscreen = true;
            surface.schedule_configure();
        },
        SetMinimized => unsafe {
            ffi::shell_surface_set_minimized(shsurf);
        },
    }
}

fn handle_xdg_popup_request(_resource: &mut Resource<XdgPopup>, request: &xdg_popup::Request) {
    use xdg_popup::Request::*;

    match request {
        Destroy => (),
        Grab { seat: _, serial: _ } => (),
        Reposition { positioner: _, token: _ } => (),
    }
}

fn handle_xdg_surface_request(resource: &mut Resource<XdgSurface>, request: &xdg_surface::Request) {
    use xdg_surface::Request::*;
    use xdg_wm_base::Error;

    let surface = resource.user_data::<Surface>();

    match request {
        Destroy => (),
        GetToplevel { id } => unsafe {
            id.on_request(handle_xdg_toplevel_request);

            let view = ffi::weston_view_create(surface.surface);

            surface.shsurf = ffi::shell_surface_create(
                &mut (surface.shell.borrow_mut()).native as *mut _,
                surface.surface,
                view,
            );
            id.set_user_data(surface as *mut _);
        },
        GetPopup { id, parent, positioner: _ } => {
            if parent.is_none() {
                resource.post_error(
                    Error::InvalidPopupParent as u32,
                    "popup parent must be non-null",
                );
                return;
            };
            id.on_request(handle_xdg_popup_request);
        }
        SetWindowGeometry { x, y, width, height } => {
            // xxx: check it's either toplevel or popup
            surface.next_geometry = Some(ffi::weston_geometry {
                x: *x,
                y: *y,
                width: *width,
                height: *height,
            })
        }
        AckConfigure { serial: _ } => {
            // xxx: check it's either toplevel or popup

            // xxx: lookup pending configure request instead of this stub
            // xxx: free older configures
            // xxx: XDG_WM_BASE_ERROR_INVALID_SURFACE_STATE if not found
            let configure_state = State::default();
            let configure_size = ffi::weston_size { width: 0, height: 0 };

            surface.configured = true;
            surface.next_state = configure_state;
            surface.next_size = configure_size;
        }
    }
}

struct Client {
    shell: Rc<RefCell<crate::Shell>>,
}

impl Client {
    fn new(rc: Rc<RefCell<crate::Shell>>) -> Self {
        Client { shell: rc }
    }
}

impl<'a> From<&'a protocol::wl_surface::WlSurface> for &'a mut ffi::weston_surface {
    fn from(item: &'a protocol::wl_surface::WlSurface) -> Self {
        item.as_ref().user_data::<ffi::weston_surface>()
    }
}

fn handle_xdg_wm_base_request(resource: &mut Resource<XdgWmBase>, request: &xdg_wm_base::Request) {
    use xdg_wm_base::Request::*;
    let client = resource.user_data::<Client>();

    match request {
        Destroy => {
            // drop resource
            debug!("destroy")
        }
        CreatePositioner { id } => {
            debug!("create positioner");
            id.on_request(handle_positioner_request);
            let b = Box::new(Positioner::new());
            id.set_user_data(Box::into_raw(b));
        }
        GetXdgSurface { id, surface } => {
            debug!("get xdg surface");
            let xs = Box::new(Surface::new(client.shell.clone(), surface.into()));

            id.on_request(handle_xdg_surface_request);
            id.set_user_data(Box::into_raw(xs) as *mut _);
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

    let rc = rc.clone();
    let global = compositor
        .server
        .create_global::<XdgWmBase>(1)
        .on_bind(move |resource| {
            /* xxx: set up ping timer */

            let c = Box::new(Client::new(rc.clone()));
            let p = Box::into_raw(c) as *mut _;

            resource.set_user_data(p);
            true
        })
        .on_request(handle_xdg_wm_base_request);

    Box::into_raw(Box::new(global));
}
