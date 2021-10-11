// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cell::RefCell;
use std::rc::Rc;

use crate::ffi;
use crate::ffi::evdev::{BTN_LEFT, KEY_ESC, KEY_F3};
use crate::Shell;

// TODO:
//
//  - Rotate on Ctrl-Alt-Shift-REFRESH
//

// #[derive(Listener)]

struct Rotation {
    transform: ffi::weston_transform,
    view_destroy_listener: ffi::wl_listener,
}

impl From<*mut ffi::wl_listener> for Box<Rotation> {
    fn from(listener: *mut ffi::wl_listener) -> Box<Rotation> {
        unsafe { Box::from_raw(container_of_mut!(listener, Rotation, view_destroy_listener)) }
    }
}

impl<'a> From<&'a mut Box<Rotation>> for &'a mut ffi::wl_listener {
    fn from(rotation: &'a mut Box<Rotation>) -> &'a mut ffi::wl_listener {
        &mut rotation.view_destroy_listener
    }
}

impl Drop for Rotation {
    fn drop(&mut self) {
        self.transform.link.remove();
    }
}

fn key_rotate(surface: &mut ffi::weston_surface) {
    let view = unsafe { &mut *ffi::get_default_view(surface as *mut _) };
    let compositor = unsafe { &mut *(*view.surface).compositor };

    let cx = surface.width as f32 * 0.5;
    let cy = surface.height as f32 * 0.5;
    let (s, c) = f32::sin_cos(0.1);

    let mut rotation = view.destroy_signal.get().unwrap_or_else(|| {
        let mut rotation = Box::new(Rotation {
            transform: ffi::weston_transform {
                matrix: ffi::weston_matrix::new(),
                link: ffi::wl_list::zeroed(),
            },
            view_destroy_listener: ffi::wl_listener::zeroed(),
        });

        view.destroy_signal.add(&mut rotation);

        unsafe {
            ffi::wl_list_insert(
                &mut view.geometry.transformation_list,
                &mut rotation.transform.link,
            );
        }

        rotation
    });

    rotation.transform.matrix = rotation
        .transform
        .matrix
        .translate(-cx, -cy, 0.0)
        .rotate(c, s)
        .translate(cx, cy, 0.0);

    view.geometry_dirty();

    compositor.schedule_repaint();

    println!("rotate {:?}", surface as *mut _)
}

fn pointer_rotate(view: &mut ffi::weston_view) {
    println!("pointer rotate {:?}", view as *mut _)
}

pub fn init(shell: &Rc<RefCell<Shell>>) {
    let shell = shell.borrow();
    let mut compositor = shell.compositor.borrow_mut();

    let mods = ffi::weston_keyboard_modifier::MODIFIER_CTRL
        | ffi::weston_keyboard_modifier::MODIFIER_ALT
        | ffi::weston_keyboard_modifier::MODIFIER_SHIFT;

    compositor.bind_key(KEY_F3, mods, move |keyboard, _, _| {
        let focus = unsafe { &mut *keyboard.focus };

        key_rotate(focus)
    });

    compositor.bind_key(KEY_ESC, mods, move |keyboard, _, _| {
        let view = unsafe { &mut *ffi::get_default_view(keyboard.focus as *mut _) };
        let compositor = unsafe { &mut *(*view.surface).compositor };

        let _ = view.destroy_signal.remove::<Box<Rotation>>();

        view.geometry_dirty();
        compositor.schedule_repaint();
    });

    compositor.bind_button(BTN_LEFT, mods, |pointer: &mut ffi::weston_pointer, _, _| {
        let focus = unsafe { &mut *pointer.focus };

        pointer_rotate(focus)
    });
}
