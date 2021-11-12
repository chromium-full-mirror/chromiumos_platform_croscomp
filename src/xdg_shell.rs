// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::ffi;
use crate::protocol;
use crate::shell;
use log::debug;
use std::any::Any;
use std::cell::RefCell;
use std::ops::DerefMut;
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
                resource.post_error(
                    xdg_positioner::Error::InvalidInput as u32,
                    "width and height must be positives and non-zero",
                );
                return;
            }
            p.size = ffi::weston_size { width, height };
        }
        SetAnchorRect { x, y, width, height } => {
            if width < 0 || height < 0 {
                resource.post_error(
                    xdg_positioner::Error::InvalidInput as u32,
                    "width and height must be non-negative",
                );
                return;
            }
            p.anchor_rect = ffi::weston_geometry { x, y, width, height };
        }
        SetAnchor { anchor } => p.anchor = anchor,
        SetGravity { gravity } => p.gravity = gravity,
        SetConstraintAdjustment { constraint_adjustment: bits } => {
            p.constraint_adjustment =
                xdg_positioner::ConstraintAdjustment::from_bits_truncate(bits);
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

fn state_to_xdg_vec(state: &State) -> Vec<u8> {
    IntoIterator::into_iter([
        (state.maximized, xdg_toplevel::State::Maximized),
        (state.fullscreen, xdg_toplevel::State::Fullscreen),
        (state.resizing, xdg_toplevel::State::Resizing),
        (state.activated, xdg_toplevel::State::Activated),
    ])
    .filter_map(|(flag, token)| if flag { Some(token as u8) } else { None })
    .collect()
}

struct Toplevel {
    shell: Rc<RefCell<crate::Shell>>,
    surface: *mut ffi::weston_surface,
    shsurf: *mut shell::Surface,
    surface_resource: Resource<XdgSurface>,
    resource: Resource<XdgToplevel>,
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

    commit_listener: Option<Box<dyn Any>>,
}

impl Toplevel {
    fn new(surface: &Surface, resource: Resource<XdgToplevel>) -> Rc<RefCell<Self>> {
        let shsurf = unsafe {
            let view = ffi::weston_view_create(surface.surface);

            shell::Surface::new(
                surface.shell.borrow_mut().deref_mut(),
                surface.surface,
                view,
            )
        };

        let toplevel_rc = Rc::new(RefCell::new(Toplevel {
            shell: Rc::clone(&surface.shell),
            surface: surface.surface,
            shsurf,
            surface_resource: surface.resource.clone(),
            resource: resource.clone(),
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

            commit_listener: None,
        }));

        let mut toplevel = toplevel_rc.borrow_mut();
        let rc = Rc::clone(&toplevel_rc);
        let commit_signal = unsafe { &mut (*toplevel.surface).commit_signal };
        toplevel.commit_listener = Some(ffi::WlListener::new(commit_signal, move || {
            let mut toplevel = rc.borrow_mut();
            if !toplevel.configured {
                toplevel.schedule_configure(Rc::clone(&rc));
            }
        }));

        drop(toplevel);
        toplevel_rc
    }

    fn schedule_configure(&mut self, rc: Rc<RefCell<Toplevel>>) {
        let shell = self.shell.borrow_mut();
        let mut compositor = shell.compositor.borrow_mut();
        debug!("schedule configure!");
        compositor.server.event_loop.when_idle(move || {
            let toplevel = rc.borrow_mut();
            let configure = protocol::xdg_toplevel::Event::Configure {
                width: toplevel.pending_size.width,
                height: toplevel.pending_size.height,
                states: state_to_xdg_vec(&toplevel.pending_state),
            };

            toplevel.resource.send(configure);

            let configure = protocol::xdg_surface::Event::Configure { serial: 0 };

            toplevel.surface_resource.send(configure);
        });
    }
}

fn handle_xdg_toplevel_request(
    resource: &mut Resource<XdgToplevel>,
    request: &xdg_toplevel::Request,
) {
    use xdg_toplevel::Request::*;

    let rc = resource.user_data_rc::<RefCell<Toplevel>>();
    let mut toplevel = rc.borrow_mut();

    let shsurf = unsafe { &mut *toplevel.shsurf };

    match request {
        Destroy => (),
        SetParent { parent: _ } => (),
        SetTitle { title } => toplevel.title = title.clone(),
        SetAppId { app_id } => toplevel.app_id = app_id.clone(),
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
            toplevel.next_max_size = ffi::weston_size { width: *width, height: *height }
        }
        SetMinSize { width, height } => {
            toplevel.next_min_size = ffi::weston_size { width: *width, height: *height }
        }
        SetMaximized => {
            toplevel.pending_state.maximized = true;
            toplevel.pending_size = shsurf.set_maximized(true);
            toplevel.schedule_configure(Rc::clone(&rc));
        }
        UnsetMaximized => {
            toplevel.pending_state.maximized = false;
            toplevel.pending_size = shsurf.set_maximized(false);
            toplevel.schedule_configure(Rc::clone(&rc));
        }
        SetFullscreen { output } => {
            let output = match output {
                Some(output) => output.as_ref().user_data::<ffi::weston_output>(),
                None => 0 as *mut _,
            };
            toplevel.pending_size = shsurf.set_fullscreen(true, output);
            toplevel.pending_state.fullscreen = true;
            toplevel.schedule_configure(Rc::clone(&rc));
        }
        UnsetFullscreen => {
            toplevel.pending_size = shsurf.set_fullscreen(false, 0 as *mut _);
            toplevel.pending_state.fullscreen = true;
            toplevel.schedule_configure(Rc::clone(&rc));
        }
        SetMinimized => {
            shsurf.set_minimized();
        }
    }
}

fn handle_xdg_toplevel_surface_request(
    resource: &mut Resource<XdgSurface>,
    request: &xdg_surface::Request,
) {
    use xdg_surface::Request::*;
    let rc = resource.user_data_rc::<RefCell<Toplevel>>();
    let mut toplevel = rc.borrow_mut();

    match request {
        Destroy => (),
        GetToplevel { .. } | GetPopup { .. } => {
            resource.post_error(
                xdg_wm_base::Error::Role as u32,
                "surface already has role XdgToplevel",
            );
            return;
        }
        SetWindowGeometry { x, y, width, height } => {
            let shsurf = unsafe { &mut *toplevel.shsurf };
            shsurf.geometry = ffi::weston_geometry {
                x: *x,
                y: *y,
                width: *width,
                height: *height,
            }
        }
        AckConfigure { serial: _ } => toplevel.configured = true,
    }
}

fn handle_xdg_popup_request(_resource: &mut Resource<XdgPopup>, request: &xdg_popup::Request) {
    use xdg_popup::Request::*;

    match request {
        Destroy => todo!(),
        Grab { seat: _, serial: _ } => todo!(),
        Reposition { positioner: _, token: _ } => todo!(),
    }
}

fn handle_xdg_popup_surface_request(
    resource: &mut Resource<XdgSurface>,
    request: &xdg_surface::Request,
) {
    use xdg_surface::Request::*;

    match request {
        Destroy => todo!(),
        GetToplevel { .. } | GetPopup { .. } => {
            resource.post_error(
                xdg_wm_base::Error::Role as u32,
                "surface already has role XdgPopup",
            );
            return;
        }
        SetWindowGeometry { x: _, y: _, width: _, height: _ } => todo!(),
        AckConfigure { serial: _ } => todo!(),
    }
}

fn handle_xdg_surface_request(resource: &mut Resource<XdgSurface>, request: &xdg_surface::Request) {
    use xdg_surface::Request::*;
    use xdg_wm_base::Error;

    let surface_rc = resource.user_data_rc::<RefCell<Surface>>();
    let surface = surface_rc.borrow_mut();

    match request {
        Destroy => (),
        GetToplevel { id } => {
            let toplevel = Toplevel::new(&surface, id.clone().into());
            id.on_request(handle_xdg_toplevel_request);
            id.set_user_data(Rc::into_raw(Rc::clone(&toplevel)));

            resource.on_request(handle_xdg_toplevel_surface_request);
            resource.set_user_data(Rc::into_raw(Rc::clone(&toplevel)));
        }
        GetPopup { id, parent, positioner: _ } => {
            if parent.is_none() {
                resource.post_error(
                    Error::InvalidPopupParent as u32,
                    "popup parent must be non-null",
                );
                return;
            };
            id.on_request(handle_xdg_popup_request);
            resource.on_request(handle_xdg_popup_surface_request);
        }
        SetWindowGeometry { .. } | AckConfigure { .. } => {
            resource.post_error(
                xdg_surface::Error::NotConstructed as u32,
                "xdg_surface must have a role",
            );
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

struct Surface {
    shell: Rc<RefCell<crate::Shell>>,
    surface: *mut ffi::weston_surface,
    resource: Resource<XdgSurface>,
}

impl Surface {
    fn new(
        shell: Rc<RefCell<crate::Shell>>,
        surface: *mut ffi::weston_surface,
        resource: Resource<XdgSurface>,
    ) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self { shell, surface, resource }))
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
            let xdg_surface = Surface::new(
                Rc::clone(&client.shell),
                surface.as_ref().user_data::<ffi::weston_surface>() as *mut _,
                id.clone().into(),
            );

            id.on_request(handle_xdg_surface_request);
            id.set_user_data(Rc::into_raw(xdg_surface));
        }
        Pong { serial: _ } => {
            todo!("update timer, end busy cursor, pong");
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
