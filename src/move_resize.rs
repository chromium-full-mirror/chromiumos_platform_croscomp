// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cell::RefCell;
use std::rc::Rc;

use crate::ffi;
use crate::input;
use crate::Shell;

struct MoveGrab {
    view_destroy_listener: ffi::wl_listener,
    dx: ffi::wl_fixed_t,
    dy: ffi::wl_fixed_t,
    pointer_grab: input::PointerGrab,
    keyboard_grab: input::KeyboardGrab,
}

impl input::PointerGrabHandler for MoveGrab {
    fn motion(
        &mut self,
        pointer: &mut ffi::weston_pointer,
        _time: &ffi::timespec,
        event: *mut ffi::weston_pointer_motion_event,
    ) {
        let view = unsafe { &mut *pointer.focus };
        let compositor = unsafe { &mut *(*view.surface).compositor };

        pointer.r#move(event);
        view.set_position((pointer.x + self.dx).into(), (pointer.y + self.dy).into());
        compositor.schedule_repaint();
    }

    fn button(
        &mut self,
        pointer: &mut ffi::weston_pointer,
        _time: &ffi::timespec,
        _button: u32,
        state: u32,
    ) {
        if pointer.button_count == 0
            && state == ffi::wl_pointer_button_state::WL_POINTER_BUTTON_STATE_RELEASED as u32
        {
            self.shutdown()
        }
    }

    fn cancel(&mut self, _pointer: &mut ffi::weston_pointer) {
        self.shutdown()
    }
}

impl input::KeyboardGrabHandler for MoveGrab {
    fn key(
        &mut self,
        keyboard: &mut ffi::weston_keyboard,
        _time: &ffi::timespec,
        key: u32,
        state: u32,
    ) {
        use crate::ffi::evdev::KEY_ESC;
        use ffi::wl_keyboard_key_state::*;

        if key == KEY_ESC && state == WL_KEYBOARD_KEY_STATE_PRESSED as u32 {
            let seat = unsafe { &mut *keyboard.seat };
            let pointer = unsafe { &mut *seat.pointer_state };
            let view = unsafe { &mut *pointer.focus };
            let compositor = unsafe { &mut *(*view.surface).compositor };

            view.set_position(
                (pointer.grab_x + self.dx).into(),
                (pointer.grab_y + self.dy).into(),
            );
            compositor.schedule_repaint();

            self.shutdown()
        }
    }

    fn cancel(&mut self, _keyboard: &mut ffi::weston_keyboard) {
        self.shutdown()
    }
}

impl MoveGrab {
    fn start(pointer: *mut ffi::weston_pointer, view: *mut ffi::weston_view) {
        let pointer = unsafe { &mut *pointer };
        let view = unsafe { &mut *view };
        let seat = unsafe { &mut *pointer.seat };
        let keyboard = unsafe { &mut *seat.keyboard_state };

        let mut handler = Box::new(MoveGrab {
            view_destroy_listener: ffi::wl_listener::new(view_destroy_notify),
            dx: ffi::wl_fixed_t::from(view.geometry.x) - pointer.grab_x,
            dy: ffi::wl_fixed_t::from(view.geometry.y) - pointer.grab_y,
            pointer_grab: input::PointerGrab::new(pointer),
            keyboard_grab: input::KeyboardGrab::new(keyboard),
        });

        let p = handler.as_mut() as *mut Self;
        handler.pointer_grab.start(p);
        handler.keyboard_grab.start(p);
        Box::into_raw(handler);
    }

    fn shutdown(&mut self) {
        // xxx this is kind of crazy, the caller still thinks self is
        // valid when we return...
        let _ = unsafe { Box::from_raw(self as *mut _) };
    }
}

struct ResizeGrab {
    view_destroy_listener: ffi::wl_listener,
    edges: ffi::wl_shell_surface_resize,
    origin_x: f32,
    origin_y: f32,
    width: i32,
    height: i32,
    pointer_grab: input::PointerGrab,
    keyboard_grab: input::KeyboardGrab,
}

impl input::PointerGrabHandler for ResizeGrab {
    fn motion(
        &mut self,
        pointer: &mut ffi::weston_pointer,
        _time: &ffi::timespec,
        event: *mut ffi::weston_pointer_motion_event,
    ) {
        let view = unsafe { &mut *pointer.focus };
        use ffi::wl_shell_surface_resize as resize;

        pointer.r#move(event);

        let (from_x, from_y): (ffi::wl_fixed_t, _) =
            view.from_global(pointer.grab_x, pointer.grab_y);
        let (to_x, to_y): (ffi::wl_fixed_t, _) = view.from_global(pointer.x, pointer.y);

        let width = self.width
            + if self.edges.has(resize::WL_SHELL_SURFACE_RESIZE_LEFT) {
                (from_x - to_x).into()
            } else if self.edges.has(resize::WL_SHELL_SURFACE_RESIZE_RIGHT) {
                (to_x - from_x).into()
            } else {
                0
            };
        let height = self.height
            + if self.edges.has(resize::WL_SHELL_SURFACE_RESIZE_TOP) {
                (from_y - to_y).into()
            } else if self.edges.has(resize::WL_SHELL_SURFACE_RESIZE_BOTTOM) {
                (to_y - from_y).into()
            } else {
                0
            };

        // xxx check max/min sizes

        let surface = unsafe { &mut *(view.surface) };
        if let Some(shsurf) = ffi::shell_surface::get(surface) {
            shsurf.set_size(width, height);
        }
    }

    fn button(
        &mut self,
        pointer: &mut ffi::weston_pointer,
        _time: &ffi::timespec,
        _button: u32,
        state: u32,
    ) {
        if pointer.button_count == 0
            && state == ffi::wl_pointer_button_state::WL_POINTER_BUTTON_STATE_RELEASED as u32
        {
            self.shutdown(pointer);
        }
    }

    fn cancel(&mut self, pointer: &mut ffi::weston_pointer) {
        self.shutdown(pointer);
    }
}
impl input::KeyboardGrabHandler for ResizeGrab {
    fn key(
        &mut self,
        keyboard: &mut ffi::weston_keyboard,
        _time: &ffi::timespec,
        key: u32,
        state: u32,
    ) {
        use crate::ffi::evdev::KEY_ESC;
        use ffi::wl_keyboard_key_state::*;

        if key == KEY_ESC && state == WL_KEYBOARD_KEY_STATE_PRESSED as u32 {
            let seat = unsafe { &mut *keyboard.seat };
            let pointer = unsafe { &mut *seat.pointer_state };
            let view = unsafe { &mut *pointer.focus };

            let surface = unsafe { &mut *(view.surface) };
            if let Some(shsurf) = ffi::shell_surface::get(surface) {
                view.set_position(self.origin_x, self.origin_y);
                shsurf.set_size(self.width, self.height);
            }

            self.shutdown(pointer)
        }
    }

    fn cancel(&mut self, keyboard: &mut ffi::weston_keyboard) {
        let seat = unsafe { &mut *keyboard.seat };
        let pointer = unsafe { &mut *seat.pointer_state };

        self.shutdown(pointer)
    }
}

impl ResizeGrab {
    fn start(
        pointer: *mut ffi::weston_pointer,
        view: *mut ffi::weston_view,
        edges: ffi::wl_shell_surface_resize,
    ) {
        let pointer = unsafe { &mut *pointer };
        let seat = unsafe { &mut *pointer.seat };
        let keyboard = unsafe { &mut *seat.keyboard_state };
        let view = unsafe { &mut *view };
        let surface = unsafe { &mut *(view.surface) };
        use ffi::wl_shell_surface_resize as resize;

        // xxx reject if shsurf->grabbed or fullscreen or maximized.

        const LEFT_RIGHT: resize = resize(1 | 2);
        const TOP_BOTTOM: resize = resize(4 | 8);

        if edges == resize::WL_SHELL_SURFACE_RESIZE_NONE
            || edges & TOP_BOTTOM == TOP_BOTTOM
            || edges & LEFT_RIGHT == LEFT_RIGHT
        {
            // Invalid edges, fail silently
            return;
        }

        let (width, height) = if let Some(shsurf) = ffi::shell_surface::get(surface) {
            let geometry = shsurf.get_geometry();
            (geometry.width, geometry.height)
        } else {
            (surface.width, surface.height)
        };

        let mut handler = Box::new(ResizeGrab {
            view_destroy_listener: ffi::wl_listener::new(view_destroy_notify),
            edges: edges,
            origin_x: view.geometry.x,
            origin_y: view.geometry.y,
            width: width,
            height: height,
            pointer_grab: input::PointerGrab::new(pointer),
            keyboard_grab: input::KeyboardGrab::new(keyboard),
        });

        if let Some(shsurf) = ffi::shell_surface::get(surface) {
            shsurf.set_resizing(edges);
        }

        let p = handler.as_mut() as *mut Self;
        handler.pointer_grab.start(p);
        handler.keyboard_grab.start(p);
        Box::into_raw(handler);
    }

    fn shutdown(&mut self, pointer: &mut ffi::weston_pointer) {
        let view = unsafe { &mut *pointer.focus };
        let surface = unsafe { &mut *(view.surface) };
        use ffi::wl_shell_surface_resize as resize;

        if let Some(shsurf) = ffi::shell_surface::get(surface) {
            shsurf.set_resizing(resize::WL_SHELL_SURFACE_RESIZE_NONE);
        }

        // xxx unset shsurf resizing
        let _ = unsafe { Box::from_raw(self as *mut _) };
    }
}

extern "C" fn view_destroy_notify(
    listener: *mut ffi::wl_listener,
    _data: *mut ::std::os::raw::c_void,
) {
    let _grab = unsafe { &mut *container_of_mut!(listener, MoveGrab, view_destroy_listener) };
}

pub fn init(shell: &Rc<RefCell<Shell>>) {
    let shell = shell.borrow();
    let mut compositor = shell.compositor.borrow_mut();

    use crate::ffi::evdev::{BTN_LEFT, BTN_RIGHT};

    compositor.bind_button(
        BTN_LEFT,
        ffi::weston_keyboard_modifier(shell.native.binding_modifier),
        |pointer, _, _| {
            if !pointer.focus.is_null() {
                MoveGrab::start(pointer, pointer.focus);
            }
        },
    );

    let resize = |pointer: &mut ffi::weston_pointer, _, _| {
        if !pointer.focus.is_null() {
            let view = unsafe { &mut *pointer.focus };
            let surface = unsafe { &mut *view.surface };
            use ffi::wl_shell_surface_resize as resize;

            let (x, y) = view.from_global::<_, f32>(pointer.grab_x, pointer.grab_y);

            let (x, y) = (x as i32, y as i32);
            let hmargin = surface.width / 3;
            let vmargin = surface.height / 3;
            let edges = if x < hmargin {
                resize::WL_SHELL_SURFACE_RESIZE_LEFT
            } else if x < surface.width - hmargin {
                resize::WL_SHELL_SURFACE_RESIZE_NONE
            } else {
                resize::WL_SHELL_SURFACE_RESIZE_RIGHT
            } | if y < vmargin {
                resize::WL_SHELL_SURFACE_RESIZE_TOP
            } else if y < surface.height - vmargin {
                resize::WL_SHELL_SURFACE_RESIZE_NONE
            } else {
                resize::WL_SHELL_SURFACE_RESIZE_BOTTOM
            };

            ResizeGrab::start(
                pointer,
                pointer.focus,
                edges as ffi::wl_shell_surface_resize,
            )
        }
    };

    compositor.bind_button(
        BTN_RIGHT,
        ffi::weston_keyboard_modifier(shell.native.binding_modifier),
        resize,
    );
    compositor.bind_button(
        BTN_LEFT,
        ffi::weston_keyboard_modifier(shell.native.binding_modifier)
            | ffi::weston_keyboard_modifier::MODIFIER_SHIFT,
        resize,
    );
}

#[no_mangle]
pub extern "C" fn move_view(view: *mut ffi::weston_view, pointer: *mut ffi::weston_pointer) {
    MoveGrab::start(pointer, view);
}

#[no_mangle]
pub extern "C" fn resize_view(
    view: *mut ffi::weston_view,
    pointer: *mut ffi::weston_pointer,
    edges: u32,
) {
    ResizeGrab::start(pointer, view, ffi::wl_shell_surface_resize(edges));
}
