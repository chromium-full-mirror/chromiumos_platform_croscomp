// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::ffi;
use crate::input;
use crate::shell;

use crate::protocol::wl_keyboard::KeyState;
use crate::protocol::wl_pointer::ButtonState;

pub struct MoveGrab {
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
        state: ButtonState,
    ) {
        if pointer.button_count == 0 && state == ButtonState::Released {
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
        state: KeyState,
    ) {
        use crate::ffi::evdev::KEY_ESC;

        if key == KEY_ESC && state == KeyState::Pressed {
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
    pub fn start(pointer: *mut ffi::weston_pointer, shsurf: &shell::Surface) {
        let pointer = unsafe { &mut *pointer };
        let view = unsafe { &mut *shsurf.view };
        let seat = unsafe { &mut *pointer.seat };
        let keyboard = unsafe { &mut *seat.keyboard_state };

        let mut handler = Box::new(MoveGrab {
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

pub struct ResizeGrab {
    edges: shell::ResizeEdge,
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

        pointer.r#move(event);

        let (from_x, from_y): (ffi::wl_fixed_t, _) =
            view.from_global(pointer.grab_x, pointer.grab_y);
        let (to_x, to_y): (ffi::wl_fixed_t, _) = view.from_global(pointer.x, pointer.y);

        let width = self.width
            + if self.edges.left {
                (from_x - to_x).into()
            } else if self.edges.right {
                (to_x - from_x).into()
            } else {
                0
            };
        let height = self.height
            + if self.edges.top {
                (from_y - to_y).into()
            } else if self.edges.bottom {
                (to_y - from_y).into()
            } else {
                0
            };

        // xxx check max/min sizes

        let surface = unsafe { &mut *(view.surface) };
        if let Some(shsurf) = shell::Surface::get(surface) {
            shsurf.borrow_mut().set_size(width, height);
        }
    }

    fn button(
        &mut self,
        pointer: &mut ffi::weston_pointer,
        _time: &ffi::timespec,
        _button: u32,
        state: ButtonState,
    ) {
        if pointer.button_count == 0 && state == ButtonState::Released {
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
        state: KeyState,
    ) {
        use crate::ffi::evdev::KEY_ESC;

        if key == KEY_ESC && state == KeyState::Pressed {
            let seat = unsafe { &mut *keyboard.seat };
            let pointer = unsafe { &mut *seat.pointer_state };
            let view = unsafe { &mut *pointer.focus };

            let surface = unsafe { &mut *(view.surface) };
            if let Some(shsurf) = shell::Surface::get(surface) {
                view.set_position(self.origin_x, self.origin_y);
                shsurf.borrow_mut().set_size(self.width, self.height);
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
    pub fn start(
        pointer: *mut ffi::weston_pointer,
        shsurf: &mut shell::Surface,
        edges: shell::ResizeEdge,
    ) {
        let pointer = unsafe { &mut *pointer };
        let seat = unsafe { &mut *pointer.seat };
        let keyboard = unsafe { &mut *seat.keyboard_state };
        let view = unsafe { &mut *shsurf.view };

        // xxx reject if shsurf->grabbed or fullscreen or maximized.

        let geometry = shsurf.get_geometry();
        let mut handler = Box::new(ResizeGrab {
            edges: edges,
            origin_x: view.geometry.x,
            origin_y: view.geometry.y,
            width: geometry.width,
            height: geometry.height,
            pointer_grab: input::PointerGrab::new(pointer),
            keyboard_grab: input::KeyboardGrab::new(keyboard),
        });

        shsurf.set_resizing(edges);

        let p = handler.as_mut() as *mut Self;
        handler.pointer_grab.start(p);
        handler.keyboard_grab.start(p);
        Box::into_raw(handler);
    }

    fn shutdown(&mut self, pointer: &mut ffi::weston_pointer) {
        let view = unsafe { &mut *pointer.focus };
        let surface = unsafe { &mut *(view.surface) };

        if let Some(shsurf) = shell::Surface::get(surface) {
            shsurf.borrow_mut().set_resizing(shell::ResizeEdge::none());
        }

        // xxx unset shsurf resizing
        let _ = unsafe { Box::from_raw(self as *mut _) };
    }
}
